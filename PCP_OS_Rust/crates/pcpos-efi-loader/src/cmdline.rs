use core::fmt::{self, Write};

pub(crate) struct Line<'a> {
    buf: &'a mut [u8],
    n: usize,
}
impl Write for Line<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.n.checked_add(s.len()).ok_or(fmt::Error)?;
        if end >= self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.n..end].copy_from_slice(s.as_bytes());
        self.buf[end] = 0;
        self.n = end;
        Ok(())
    }
}
impl<'a> Line<'a> {
    pub(crate) fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, n: 0 }
    }

    pub(crate) fn append_rsdp(&mut self, mut table: impl FnMut(bool) -> u64) -> fmt::Result {
        // Leave the required words and their terminator intact if the hint cannot fit.
        if self.buf.len() - self.n <= b" acpi_rsdp=0x0000000000000000".len() {
            return Ok(());
        }
        let modern = table(true);
        let address = if modern != 0 { modern } else { table(false) };
        if address == 0 {
            return Ok(());
        }
        write!(self, " acpi_rsdp=0x{address:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_modern_table_and_keeps_the_full_address() {
        let mut buf = [0xa5; 48];
        let mut line = Line::new(&mut buf);
        line.write_str("auto=x").unwrap();
        line.append_rsdp(|modern| {
            assert!(modern);
            0xfedc_ba98_7654_3210
        })
        .unwrap();
        assert_eq!(&buf[..36], b"auto=x acpi_rsdp=0xfedcba9876543210\0");
    }

    #[test]
    fn legacy_table_and_absence() {
        let mut buf = [0xa5; 48];
        let mut line = Line::new(&mut buf);
        line.write_str("x").unwrap();
        line.append_rsdp(|modern| if modern { 0 } else { 0xab })
            .unwrap();
        assert_eq!(&buf[..31], b"x acpi_rsdp=0x00000000000000ab\0");
        let mut buf = [0xa5; 48];
        let mut line = Line::new(&mut buf);
        line.write_str("x").unwrap();
        let mut calls = Vec::new();
        line.append_rsdp(|modern| {
            calls.push(modern);
            0
        })
        .unwrap();
        assert_eq!(calls, [true, false]);
        assert_eq!(&buf[..2], b"x\0");
        assert!(buf[2..].iter().all(|b| *b == 0xa5));
    }

    #[test]
    fn optional_word_fits_whole_or_preserves_the_existing_line() {
        let word = b" acpi_rsdp=0x0000000000000001";
        for spare in [word.len(), word.len() + 1] {
            let mut buf = [0xa5; 1024];
            let prefix = "x".repeat(buf.len() - spare);
            let mut line = Line::new(&mut buf);
            line.write_str(&prefix).unwrap();
            line.append_rsdp(|_| 1).unwrap();
            assert_eq!(&buf[..prefix.len()], prefix.as_bytes());
            if spare == word.len() {
                assert_eq!(buf[prefix.len()], 0);
                assert!(buf[prefix.len() + 1..].iter().all(|b| *b == 0xa5));
            } else {
                assert_eq!(&buf[prefix.len()..1023], word);
                assert_eq!(buf[1023], 0);
            }
        }
    }
}
