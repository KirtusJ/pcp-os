//! A finite direct peer link for exactly three slots per direction. A slot is never reused.
use super::{
    completion, descriptor, with_queues, BusDevice, Queue, FRAME_BYTES, HEADER, RX, RX_CAPACITY,
};
use super::{in8, out16, out8};
use crate::arch;
use core::sync::atomic::{fence, Ordering};

const SLOTS: usize = 3;
const STRIDE: usize = 80;

struct Ring<'a> {
    queue: &'a Queue,
    first: usize,
    posted: usize,
    completed: usize,
}

impl Ring<'_> {
    fn setup(&self, io: u16) -> Result<(), &'static str> {
        if self.queue.size < 2 * SLOTS {
            return Err("worker queue too small");
        }
        self.queue.install(io, self.first)?;
        Ok(())
    }

    fn post(&mut self, io: u16, frame: Option<&[u8; FRAME_BYTES]>) -> Result<(), &'static str> {
        let slot = self.posted;
        if slot >= SLOTS {
            return Err("worker post limit");
        }
        let rx = self.queue.index == RX;
        if rx != frame.is_none() {
            return Err("worker queue direction");
        }
        // Three independent RX windows, two in the dedicated data page and a third
        // in an extra reserved page. TX windows need only 80 bytes apiece.
        let stride = if rx { 1536 } else { STRIDE };
        let data_base = self.queue.data(self.first);
        let data = data_base + if rx && slot == 2 { 4096 } else { slot * stride };
        let page = data & !4095;
        let offset = data & 4095;
        if let Some(bytes) = frame {
            if !arch::frame_write(page, offset + 16, bytes) {
                return Err("worker packet initialization");
            }
        } else if !arch::frame_write(page, offset, &[0xa5; 16 + FRAME_BYTES]) {
            return Err("worker receive initialization");
        }
        let head = (2 * slot) as u16;
        let size = if rx { RX_CAPACITY } else { FRAME_BYTES };
        let (avail, off) = self.queue.available(self.first);
        if !arch::frame_write(
            self.first,
            16 * head as usize,
            &descriptor(data, HEADER, Some(head + 1), rx),
        ) || !arch::frame_write(
            self.first,
            16 * (head as usize + 1),
            &descriptor(data + 16, size, None, rx),
        ) || !arch::frame_write(avail, off + 4 + 2 * slot, &head.to_le_bytes())
        {
            return Err("worker descriptor initialization");
        }
        fence(Ordering::SeqCst);
        if !arch::frame_publish_u16(avail, off + 2, (slot + 1) as u16) {
            return Err("worker available index");
        }
        fence(Ordering::SeqCst);
        out16(io + 16, self.queue.index);
        self.posted += 1;
        Ok(())
    }

    fn wait(&mut self, polls: usize) -> Result<usize, &'static str> {
        let used = self.first + self.queue.used_offset;
        let slot = self.completed;
        if slot >= self.posted {
            return Err("worker completion without buffer");
        }
        for _ in 0..polls {
            // The coherent device owns the used ring until this index changes.
            let count = unsafe { core::ptr::read_volatile((used + 2) as *const u16) } as usize;
            if count != slot {
                if count < slot || count > self.posted {
                    return Err("worker invalid used count");
                }
                fence(Ordering::Acquire);
                let at = used + 4 + 8 * slot;
                let head = unsafe { core::ptr::read_volatile(at as *const u32) };
                let len = unsafe { core::ptr::read_volatile((at + 4) as *const u32) };
                // This single-queue fixture orders its packet exchange. No head from
                // the device becomes an address. The driver uses its local slot.
                if head != (slot * 2) as u32 {
                    return Err("worker unexpected used head");
                }
                self.completed += 1;
                return Ok(len as usize);
            }
            core::hint::spin_loop();
        }
        Err(if self.queue.index == RX {
            "worker RX timeout"
        } else {
            "worker TX timeout"
        })
    }
}

pub(crate) struct Link<'a> {
    io: u16,
    tx: Ring<'a>,
    rx: Ring<'a>,
}

impl Link<'_> {
    pub(crate) fn send(&mut self, bytes: &[u8; FRAME_BYTES]) -> Result<(), &'static str> {
        self.tx.post(self.io, Some(bytes))?;
        self.tx.wait(10_000_000)?;
        Ok(())
    }
    pub(crate) fn receive(&mut self) -> Result<[u8; FRAME_BYTES], &'static str> {
        let slot = self.rx.completed;
        let len = self
            .rx
            .wait(if slot == 0 { 200_000_000 } else { 50_000_000 })?;
        if len != HEADER + FRAME_BYTES {
            return Err("worker frame length");
        }
        let base = self.rx.queue.data(self.rx.first);
        let data = base + if slot == 2 { 4096 } else { slot * 1536 };
        let header =
            core::array::from_fn(|i| unsafe { core::ptr::read_volatile((data + i) as *const u8) });
        completion::validate_header(&header)?;
        Ok(core::array::from_fn(|i| unsafe {
            core::ptr::read_volatile((data + 16 + i) as *const u8)
        }))
    }
}

pub(crate) fn connected<T>(
    bus: &[BusDevice],
    mac_last: u8,
    task: impl FnOnce(&mut Link) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    with_queues(bus, mac_last, true, 1, |io, first, tx, rx| {
        let rx = rx.ok_or("worker missing RX queue")?;
        let mut link = Link {
            io,
            tx: Ring {
                queue: tx,
                first,
                posted: 0,
                completed: 0,
            },
            rx: Ring {
                queue: rx,
                first: first + tx.pages * 4096,
                posted: 0,
                completed: 0,
            },
        };
        link.tx.setup(io)?;
        link.rx.setup(io)?;
        out8(io + 18, 7);
        if in8(io + 18) != 7 {
            return Err("worker driver status readback");
        }
        for _ in 0..SLOTS {
            link.rx.post(io, None)?;
        }
        task(&mut link)
    })
}
