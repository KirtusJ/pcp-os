//! Session-scoped packet I/O. Each ring has one outstanding chain (head zero).
//! The existing PCI setup owns every DMA page until the program exits and reset completes.
use super::{completion, descriptor, with_queues, Queue, HEADER, RX_CAPACITY};
use super::udp_policy::{Counts, Rule, Session};
use crate::{arch, dev};
use core::sync::atomic::{fence, Ordering};

pub(crate) const MTU: usize = 1514;
type Error = &'static str;

struct Ring {
    queue: Queue,
    base: usize,
    done: u16,
}

impl Ring {
    fn post(&self, io: u16) -> Result<(), Error> {
        let (page, offset) = self.queue.available(self.base);
        let slot = self.done as usize % self.queue.size;
        if !arch::frame_write(page, offset + 4 + slot * 2, &0u16.to_le_bytes()) {
            return Err("available head");
        }
        fence(Ordering::Release);
        if !arch::frame_publish_u16(page, offset + 2, self.done.wrapping_add(1)) {
            return Err("available index");
        }
        fence(Ordering::SeqCst);
        super::out16(io + 16, self.queue.index);
        Ok(())
    }
    fn completion(&mut self) -> Result<Option<usize>, Error> {
        let used = self.base + self.queue.used_offset;
        // The device writes these aligned fields in its reserved, coherent used ring.
        let count = unsafe { core::ptr::read_volatile((used + 2) as *const u16) };
        if count == self.done {
            return Ok(None);
        }
        if count != self.done.wrapping_add(1) {
            return Err("unexpected completion count");
        }
        fence(Ordering::Acquire);
        let at = used + 4 + (self.done as usize % self.queue.size) * 8;
        let head = unsafe { core::ptr::read_volatile(at as *const u32) };
        if head != 0 {
            return Err("unexpected completion head");
        }
        let length = unsafe { core::ptr::read_volatile((at + 4) as *const u32) };
        self.done = count;
        Ok(Some(length as usize))
    }
}

struct Device {
    io: u16,
    tx: Ring,
    rx: Ring,
    stopped: bool,
    policy: Option<Session>,
}
static DEVICE: arch::Single<Option<Device>> = arch::Single::new(None);

// Caller has interrupts off. Never retain the Single borrow across execution in user mode.
pub(crate) fn during<T>(rule: Option<Rule>, run: impl FnOnce() -> T) -> (T, Result<(), Error>, Option<Counts>) {
    let mut task = Some(run);
    let mut result = None;
    let mut counts = None;
    let status = with_queues(&dev::bus_scan(), 1, true, 0, |io, first, tx, rx| {
        let rx = rx.ok_or("missing RX")?;
        let rx_base = first + tx.pages * 4096;
        tx.install(io, first)?;
        rx.install(io, rx_base)?;
        let device = Device {
            io,
            tx: Ring {
                queue: *tx,
                base: first,
                done: 0,
            },
            rx: Ring {
                queue: *rx,
                base: rx_base,
                done: 0,
            },
            stopped: false,
            policy: rule.map(Session::new),
        };
        super::out8(io + 18, 7);
        if super::in8(io + 18) != 7 {
            return Err("driver status");
        }
        device.rx.post(io)?;
        DEVICE.with(|d| *d = Some(device));
        result = Some(task.take().unwrap()());
        DEVICE.with(|d| {
            let device = d.take().unwrap();
            counts = device.policy.map(|p| p.counts);
            if device.stopped {
                Err("packet queue stopped")
            } else {
                Ok(())
            }
        })
    });
    // If setup failed before admission the process can still report an I/O failure.
    // Reset errors after execution must never cause a second launch.
    (result.unwrap_or_else(|| task.take().unwrap()()), status, counts)
}

fn access<T>(f: impl FnOnce(&mut Device) -> Result<T, Error>) -> Result<T, Error> {
    DEVICE.with(|slot| {
        let dev = slot.as_mut().ok_or("no active Ethernet device")?;
        if dev.stopped {
            return Err("packet queue stopped");
        }
        let r = f(dev);
        if r.is_err() {
            dev.stopped = true;
        }
        r
    })
}

/// False is a policy refusal. The ring remains healthy for the next request.
pub(crate) fn send(bytes: &[u8]) -> Result<bool, Error> {
    access(|d| {
        if let Some(p) = &mut d.policy {
            if !p.rule.permits(bytes, false) {
                p.counts.tx_deny = p.counts.tx_deny.saturating_add(1);
                return Ok(false);
            }
        }
        let tx = &mut d.tx;
        let data = tx.queue.data(tx.base);
        if !(14..=MTU).contains(&bytes.len())
            || !arch::frame_write(data, 16, bytes)
            || !arch::frame_write(
                tx.base,
                16,
                &descriptor(data + 16, bytes.len(), None, false),
            )
        {
            return Err("TX descriptor");
        }
        tx.post(d.io)?;
        for _ in 0..super::POLLS {
            if tx.completion()?.is_some() {
                if let Some(p) = &mut d.policy {
                    // Discard a previously completed RX descriptor before issuing
                    // a new reply credit. The independent queues supply no packet
                    // timestamps, so an already-completed fast reply is also lost.
                    if let Some(length) = d.rx.completion()? {
                        if !(HEADER..=HEADER + RX_CAPACITY).contains(&length) {
                            return Err("RX completion length");
                        }
                        d.rx.post(d.io)?;
                        p.counts.rx_drop = p.counts.rx_drop.saturating_add(1);
                    }
                    p.completed_send(arch::ticks(), 5 * arch::TICKS_PER_SECOND);
                }
                return Ok(true);
            }
            core::hint::spin_loop();
        }
        Err("TX timeout")
    })
}

pub(crate) fn receive(bytes: &mut [u8; MTU]) -> Result<Option<usize>, Error> {
    access(|d| {
        let rx = &mut d.rx;
        let Some(length) = rx.completion()? else {
            return Ok(None);
        };
        let data = rx.queue.data(rx.base);
        let minimum = if d.policy.is_some() { HEADER } else { HEADER + 14 };
        if !(minimum..=HEADER + RX_CAPACITY).contains(&length) {
            return Err("RX completion length");
        }
        let header =
            core::array::from_fn(|i| unsafe { core::ptr::read_volatile((data + i) as *const u8) });
        completion::validate_header(&header)?;
        let size = length - HEADER;
        // Drop an oversize frame; it cannot escape the kernel's reserved receive buffer.
        if size <= MTU {
            for (i, b) in bytes[..size].iter_mut().enumerate() {
                *b = unsafe { core::ptr::read_volatile((data + 16 + i) as *const u8) };
            }
        }
        rx.post(d.io)?;
        let packet = bytes.get(..size);
        if let Some(p) = &mut d.policy {
            if !p.receive(packet.unwrap_or(&[]), arch::ticks()) { return Ok(None); }
        }
        Ok(packet.map(<[u8]>::len))
    })
}
