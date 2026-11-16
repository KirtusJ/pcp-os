//! Opt-in one-shot Ethernet probes for the QEMU legacy PCI transport.
use super::{bus_read, in16, in32, in8, out16, out32, out8, BusDevice};
use crate::{arch, mem};
use core::sync::atomic::{fence, Ordering};
pub(crate) mod completion;
#[cfg(feature = "guest-ethernet")]
pub(crate) mod guest;
#[cfg(feature = "guest-ethernet")]
pub(crate) mod udp_policy;
#[cfg(feature = "virtio-worker-probe")]
pub(crate) mod worker_link;

const TX: u16 = 1;
const MAC: u32 = 1 << 5;
const FRAME_BYTES: usize = 60;

fn pci_command(d: &BusDevice, value: u16) {
    // Write just the command halfword: its neighbour is a write-one-to-clear status.
    out32(
        0xcf8,
        0x8000_0004 | (d.bus as u32) << 16 | (d.slot as u32) << 11 | (d.function as u32) << 8,
    );
    out16(0xcfc, value);
}

fn reset(io: u16) -> bool {
    out8(io + 18, 0);
    for _ in 0..100_000 {
        if in8(io + 18) == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn descriptor(address: usize, bytes: usize, next: Option<u16>, write: bool) -> [u8; 16] {
    let mut d = [0; 16];
    d[0..8].copy_from_slice(&(address as u64).to_le_bytes());
    d[8..12].copy_from_slice(&(bytes as u32).to_le_bytes());
    d[12..14].copy_from_slice(&(u16::from(next.is_some()) | u16::from(write) << 1).to_le_bytes());
    d[14..16].copy_from_slice(&next.unwrap_or(0).to_le_bytes());
    d
}

const RX: u16 = 0;
const HEADER: usize = 10;
// One legacy header and 1516 payload bytes meet the 1526-byte receive-buffer recommendation.
const RX_CAPACITY: usize = 1516;
const POLLS: usize = 10_000_000;

#[derive(Clone, Copy)]
struct Queue {
    index: u16,
    size: usize,
    used_offset: usize,
    pages: usize,
}

impl Queue {
    fn inspect(io: u16, index: u16) -> Result<Self, &'static str> {
        out16(io + 14, index);
        let size = in16(io + 12) as usize;
        if !(2..=256).contains(&size) || !size.is_power_of_two() || in32(io + 8) != 0 {
            return Err("unavailable or oversized queue");
        }
        let used_offset = (16 * size + 6 + 2 * size).next_multiple_of(4096);
        let pages = (used_offset + 6 + 8 * size).div_ceil(4096) + 1;
        Ok(Self {
            index,
            size,
            used_offset,
            pages,
        })
    }

    fn data(&self, first: usize) -> usize {
        first + 4096 * (self.pages - 1)
    }

    fn available(&self, first: usize) -> (usize, usize) {
        let offset = 16 * self.size;
        (first + (offset & !4095), offset & 4095)
    }

    fn install(&self, io: u16, first: usize) -> Result<(), &'static str> {
        let data = self.data(first);
        let rx = self.index == RX;
        let payload = if rx { RX_CAPACITY } else { FRAME_BYTES };
        let (avail, offset) = self.available(first);
        // Seed the checked window with a sentinel unlike any expected byte. In particular
        // zero-valued payload bytes cannot come from stale zero-filled memory.
        if rx && !arch::frame_write(data, 0, &[0xa5; 16 + RX_CAPACITY]) {
            return Err("receive buffer initialization");
        }
        // The legacy header is an entire descriptor. The six-byte physical gap never goes on the wire.
        if !arch::frame_write(first, 0, &descriptor(data, HEADER, Some(1), rx))
            || !arch::frame_write(first, 16, &descriptor(data + 16, payload, None, rx))
            || !arch::frame_write(avail, offset, &1u16.to_le_bytes())
        {
            return Err("ring initialization");
        }
        // Zeroed avail.ring[0] names descriptor 0; flags=NO_INTERRUPT. Order all queue
        // initialization before exposing its PFN to the coherent, legacy x86 device.
        fence(Ordering::SeqCst);
        out16(io + 14, self.index);
        out32(io + 8, (first >> 12) as u32);
        if in32(io + 8) != (first >> 12) as u32 {
            return Err("queue address readback");
        }
        Ok(())
    }

    fn publish(&self, io: u16, first: usize) -> Result<(), &'static str> {
        let (avail, offset) = self.available(first);
        fence(Ordering::SeqCst);
        if !arch::frame_publish_u16(avail, offset + 2, 1) {
            return Err("available index");
        }
        fence(Ordering::SeqCst);
        out16(io + 16, self.index);
        Ok(())
    }

    fn complete(&self, first: usize) -> Result<u32, &'static str> {
        let used = first + self.used_offset;
        for _ in 0..POLLS {
            // Safety: the queue owns the allocated, page-aligned range through confirmed reset.
            // The device writes the used index and its sole element; only these fields are polled.
            let count = unsafe { core::ptr::read_volatile((used + 2) as *const u16) };
            if count != 0 {
                fence(Ordering::Acquire);
                let head = unsafe { core::ptr::read_volatile((used + 4) as *const u32) };
                completion::validate_used(count, head)?;
                return Ok(unsafe { core::ptr::read_volatile((used + 8) as *const u32) });
            }
            core::hint::spin_loop();
        }
        Err(if self.index == RX {
            "RX completion timeout"
        } else {
            "completion timeout"
        })
    }
}

fn frame(reply: bool, nonce: [u8; 8]) -> [u8; FRAME_BYTES] {
    let mut bytes = [0; FRAME_BYTES];
    bytes[0] = 2;
    bytes[6] = 2;
    bytes[5] = if reply { 1 } else { 2 };
    bytes[11] = if reply { 2 } else { 1 };
    bytes[12..14].copy_from_slice(&0x88b5u16.to_be_bytes());
    bytes[14..34].copy_from_slice(if reply {
        b"PCP-OS virtio RX v1!"
    } else {
        b"PCP-OS virtio TX v1!"
    });
    bytes[52..60].copy_from_slice(&nonce);
    bytes
}

fn exchange(
    io: u16,
    first: usize,
    tx: &Queue,
    rx: Option<&Queue>,
    request: &[u8; FRAME_BYTES],
    reply: Option<&mut [u8]>,
) -> Result<usize, &'static str> {
    let rx_first = first + tx.pages * 4096;
    if !arch::frame_write(tx.data(first), 16, request) {
        return Err("packet initialization");
    }
    tx.install(io, first)?;
    if let Some(queue) = rx {
        queue.install(io, rx_first)?;
    }
    out8(io + 18, 7); // ACKNOWLEDGE | DRIVER | DRIVER_OK
    if in8(io + 18) != 7 {
        return Err("driver status readback");
    }
    if let Some(queue) = rx {
        queue.publish(io, rx_first)?;
    }
    tx.publish(io, first)?;
    tx.complete(first)?;
    if let (Some(queue), Some(reply)) = (rx, reply) {
        let len = queue.complete(rx_first)? as usize;
        // All limits are checked before reading the header or indexing the reserved page.
        let n = len.checked_sub(HEADER).ok_or("RX used length")?;
        if n < 14 || n > RX_CAPACITY || n > reply.len() {
            return Err("RX used length");
        }
        let data = queue.data(rx_first);
        let header =
            core::array::from_fn(|i| unsafe { core::ptr::read_volatile((data + i) as *const u8) });
        completion::validate_header(&header)?;
        // The acquired completion returns ownership. Copy only the published length; the
        // device is reset before the caller receives these bounded ordinary-memory bytes.
        for (i, byte) in reply[..n].iter_mut().enumerate() {
            *byte = unsafe { core::ptr::read_volatile((data + 16 + i) as *const u8) };
        }
        return Ok(n);
    }
    Ok(0)
}

pub fn transmit(bus: &[BusDevice]) -> Result<usize, &'static str> {
    run(bus, &frame(false, [0; 8]), None).map(|(size, _)| size)
}

#[cfg(feature = "virtio-rx-probe")]
pub fn reply(bus: &[BusDevice], cmdline: &str) -> Result<(), &'static str> {
    let nonce = completion::parse_nonce(cmdline)?;
    let expected = frame(true, nonce);
    let mut bytes = [0; FRAME_BYTES];
    let (_, n) = run(bus, &frame(false, nonce), Some(&mut bytes))?;
    completion::validate_reply_length((HEADER + n) as u32)?;
    if bytes[..52] != expected[..52] {
        return Err("RX payload mismatch");
    }
    if bytes[52..] != nonce {
        return Err("RX nonce mismatch");
    }
    Ok(())
}

#[cfg(feature = "virtio-register-probe")]
pub fn register_frame(
    bus: &[BusDevice],
    request: &[u8; FRAME_BYTES],
    reply: &mut [u8],
) -> Result<usize, &'static str> {
    run(bus, request, Some(reply)).map(|(_, n)| n)
}

fn run(
    bus: &[BusDevice],
    request: &[u8; FRAME_BYTES],
    reply: Option<&mut [u8]>,
) -> Result<(usize, usize), &'static str> {
    with_queues(bus, 1, reply.is_some(), 0, |io, first, tx, rx| {
        exchange(io, first, tx, rx, request, reply).map(|n| (tx.size, n))
    })
}

fn with_queues<T>(
    bus: &[BusDevice],
    mac_last: u8,
    receive: bool,
    extra_rx_pages: usize,
    body: impl FnOnce(u16, usize, &Queue, Option<&Queue>) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    let matches =
        |d: &&BusDevice| (d.vendor, d.device, d.class, d.subclass) == (0x1af4, 0x1000, 2, 0);
    let mut devices = bus.iter().filter(matches);
    let d = devices.next().ok_or("no legacy virtio-net PCI function")?;
    if devices.next().is_some() {
        return Err("ambiguous network functions");
    }
    let bar = d.bars[0];
    let base = bar & !3;
    if bar & 3 != 1 || !(0x1000..=0xffe0).contains(&base) {
        return Err("invalid legacy I/O BAR");
    }
    let io = base as u16;
    let command = bus_read(d.bus, d.slot, d.function, 4) as u16;
    if command & 4 != 0 {
        return Err("bus master already enabled");
    }
    // Enable just I/O decode, mask INTx; do not enable DMA until validation ends.
    pci_command(d, command | 0x401);
    let result = (|| {
        if !reset(io) {
            return Err("initial reset");
        }
        out8(io + 18, 1);
        out8(io + 18, 3);
        if in32(io) & MAC == 0 {
            return Err("MAC feature absent");
        }
        out32(io + 4, MAC);
        if in32(io + 4) != MAC {
            return Err("guest feature readback");
        }
        // This fixture explicitly disables MSI-X, hence the config starts at 20.
        let mac: [u8; 6] = core::array::from_fn(|n| in8(io + 20 + n as u16));
        if mac != [0x02, 0, 0, 0, 0, mac_last] {
            return Err("fixture MAC mismatch");
        }
        let tx = Queue::inspect(io, TX)?;
        let rx = if receive {
            Some(Queue::inspect(io, RX)?)
        } else {
            None
        };
        let pages = tx.pages + rx.as_ref().map_or(0, |q| q.pages) + extra_rx_pages;
        let first = mem::take_run(pages).ok_or("no bounded contiguous frames")?;
        let answer = if (first >> 12) + pages - 1 > u32::MAX as usize {
            Err("physical queue address exceeds legacy PFN")
        } else {
            pci_command(d, command | 0x405);
            body(io, first, &tx, rx.as_ref())
        };
        let stopped = reset(io);
        pci_command(d, command | 0x401); // turn off bus mastering before release
        if !stopped || bus_read(d.bus, d.slot, d.function, 4) & 4 != 0 {
            // A device that cannot confirm quiescence must never regain access to
            // these frames once they have been reused for an unrelated purpose.
            return Err("unconfirmed stop; DMA frames retained");
        }
        for n in 0..pages {
            mem::give(first + 4096 * n);
        }
        answer
    })();
    // A failed pre-DMA negotiation may still have set the DRIVER status.
    reset(io);
    pci_command(d, command);
    result
}
