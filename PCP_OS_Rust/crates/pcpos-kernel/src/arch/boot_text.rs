extern crate alloc;
use self::alloc::string::String;

/// Preserve the existing display prefix, and separately report whether it represents every byte
/// before an observed NUL. Only an exact string can authorize raw Ethernet.
pub(crate) fn read(mut byte_at: impl FnMut(usize) -> Option<u8>) -> (String, bool) {
    let mut text = String::new();
    let mut exact = true;
    for offset in 0..=255 {
        match byte_at(offset) {
            Some(0) => return (text, exact),
            Some(b) if offset < 255 => {
                let printable = (0x20..0x7f).contains(&b);
                exact &= printable;
                text.push(if printable { b as char } else { '?' });
            }
            _ => break,
        }
    }
    (text, false)
}
