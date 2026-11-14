//! One polled Ethernet transmit on the QEMU legacy virtio-net PCI fixture.
//! Only built for the opt-in probe; there is no receive queue or user API.
use super::{bus_read, in16, in32, in8, out16, out32, out8, BusDevice};
use crate::{arch, mem};
use core::sync::atomic::{fence, Ordering};

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

fn descriptor(address: usize, bytes: usize, next: Option<u16>) -> [u8; 16] {
    let mut d = [0; 16];
    d[0..8].copy_from_slice(&(address as u64).to_le_bytes());
    d[8..12].copy_from_slice(&(bytes as u32).to_le_bytes());
    d[12..14].copy_from_slice(&u16::from(next.is_some()).to_le_bytes());
    d[14..16].copy_from_slice(&next.unwrap_or(0).to_le_bytes());
    d
}

fn send(
    io: u16,
    qsize: usize,
    first: usize,
    data: usize,
    used: usize,
    mac: [u8; 6],
) -> Result<(), &'static str> {
    let mut frame = [0; FRAME_BYTES];
    frame[..6].copy_from_slice(&[0x02, 0, 0, 0, 0, 2]);
    frame[6..12].copy_from_slice(&mac);
    frame[12..14].copy_from_slice(&0x88b5u16.to_be_bytes());
    frame[14..34].copy_from_slice(b"PCP-OS virtio TX v1!");
    // Legacy framing with no offloads: a separate, zeroed 10-byte virtio-net
    // header, followed by the Ethernet frame. Both descriptors are read-only.
    let available = 16 * qsize;
    let apage = first + (available & !4095);
    let offset = available & 4095;
    if !arch::frame_write(data, 16, &frame)
        || !arch::frame_write(first, 0, &descriptor(data, 10, Some(1)))
        || !arch::frame_write(first, 16, &descriptor(data + 16, FRAME_BYTES, None))
        || !arch::frame_write(apage, offset, &1u16.to_le_bytes())
    {
        return Err("ring initialization");
    }
    // avail.ring[0] names descriptor 0 (zero-filled), flags=NO_INTERRUPT.
    out32(io + 8, (first >> 12) as u32);
    if in32(io + 8) != (first >> 12) as u32 {
        return Err("queue address readback");
    }
    out8(io + 18, 7); // ACKNOWLEDGE | DRIVER | DRIVER_OK (legacy transport)
    if in8(io + 18) != 7 {
        return Err("driver status readback");
    }
    fence(Ordering::SeqCst);
    if !arch::frame_write(apage, offset + 2, &1u16.to_le_bytes()) {
        return Err("available index");
    }
    fence(Ordering::SeqCst);
    out16(io + 16, TX);
    for _ in 0..10_000_000 {
        // The used ring is inside an exclusively reserved run of physical pool
        // frames. The device updates the aligned index and element; poll without
        // issuing a second descriptor. The run is kept until reset is confirmed.
        let count = unsafe { core::ptr::read_volatile((used + 2) as *const u16) };
        if count != 0 {
            fence(Ordering::Acquire);
            let head = unsafe { core::ptr::read_volatile((used + 4) as *const u32) };
            return if count == 1 && head == 0 {
                Ok(())
            } else {
                Err("invalid used entry")
            };
        }
        core::hint::spin_loop();
    }
    Err("completion timeout")
}

pub fn transmit(bus: &[BusDevice]) -> Result<usize, &'static str> {
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
        if mac != [0x02, 0, 0, 0, 0, 1] {
            return Err("fixture MAC mismatch");
        }
        out16(io + 14, TX);
        let qsize = in16(io + 12) as usize;
        if !(2..=256).contains(&qsize) || !qsize.is_power_of_two() || in32(io + 8) != 0 {
            return Err("unavailable or oversized TX queue");
        }
        // The legacy device fixes ring size and 4 KiB used-ring alignment.
        let used_offset = (16 * qsize + 6 + 2 * qsize).next_multiple_of(4096);
        let ring_pages = (used_offset + 6 + 8 * qsize).div_ceil(4096);
        let pages = ring_pages + 1; // one additional frame holds header and packet
        let first = mem::take_run(pages).ok_or("no bounded contiguous frames")?;
        let answer = if (first >> 12) > u32::MAX as usize {
            Err("physical queue address exceeds legacy PFN")
        } else {
            pci_command(d, command | 0x405);
            send(
                io,
                qsize,
                first,
                first + 4096 * ring_pages,
                first + used_offset,
                mac,
            )
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
        answer.map(|_| qsize)
    })();
    // A failed pre-DMA negotiation may still have set the DRIVER status.
    reset(io);
    pci_command(d, command);
    result
}
