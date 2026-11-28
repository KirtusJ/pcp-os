use crate::editor::CAP;
use pcp_game_guest::{call, open, sleep_us, time_us};

pub struct Clipboard {
    link: Option<usize>,
    data: [u8; CAP],
    len: usize,
}
impl Clipboard {
    pub const fn new() -> Self {
        Self {
            link: None,
            data: [0; CAP],
            len: 0,
        }
    }
    pub fn connect(&mut self) {
        self.link = usize::try_from(open(b"/dev/game-link\0", 2 | 0x800))
            .ok()
            .filter(|&fd| Self::send(fd, b"PCP-EDITOR-CLIP/1\n").is_ok());
    }
    pub fn shared(&self) -> bool {
        self.link.is_some()
    }
    fn send(fd: usize, mut bytes: &[u8]) -> Result<(), ()> {
        let mut deadline = time_us() + 5_000_000;
        while !bytes.is_empty() {
            let n = call(1, fd, bytes.as_ptr() as usize, bytes.len().min(64), 0, 0, 0);
            if n > 0 {
                deadline = time_us() + 5_000_000;
                bytes = &bytes[n as usize..];
            } else if n == -11 || n == 0 {
                sleep_us(1000);
            } else {
                return Err(());
            }
            if time_us() > deadline {
                return Err(());
            }
        }
        Ok(())
    }
    fn receive(fd: usize, mut bytes: &mut [u8]) -> Result<(), ()> {
        let mut deadline = time_us() + 5_000_000;
        while !bytes.is_empty() {
            let n = call(
                0,
                fd,
                bytes.as_mut_ptr() as usize,
                bytes.len().min(64),
                0,
                0,
                0,
            );
            if n > 0 {
                deadline = time_us() + 5_000_000;
                bytes = &mut bytes[n as usize..];
            } else if n == -11 {
                sleep_us(1000);
            } else {
                return Err(());
            }
            if time_us() > deadline {
                return Err(());
            }
        }
        Ok(())
    }
    pub fn put(&mut self, text: &[u8]) -> Result<(), ()> {
        if let Some(fd) = self.link {
            Self::send(fd, b"S")?;
            Self::send(fd, &(text.len() as u32).to_le_bytes())?;
            Self::send(fd, text)?;
            let mut status = [0];
            Self::receive(fd, &mut status)?;
            if status != [1] {
                return Err(());
            }
        }
        self.data[..text.len()].copy_from_slice(text);
        self.len = text.len();
        Ok(())
    }
    pub fn get(&mut self) -> Result<&[u8], ()> {
        if let Some(fd) = self.link {
            Self::send(fd, b"G")?;
            let mut header = [0; 4];
            Self::receive(fd, &mut header)?;
            let n = u32::from_le_bytes(header) as usize;
            if n > CAP {
                return Err(());
            }
            Self::receive(fd, &mut self.data[..n])?;
            self.len = n;
        }
        Ok(&self.data[..self.len])
    }
}
