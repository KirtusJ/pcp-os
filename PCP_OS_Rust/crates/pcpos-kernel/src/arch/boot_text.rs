extern crate alloc;
use self::alloc::string::String;

/// The most bytes of the loader's own line that are taken. It was 255, which three SHA-256 values and
/// their names (the kernel's measurement, a nominated program, a model's descriptor) do not fit in
/// together; a line that does not fit is not taken whole, and then nothing it names is acted on.
pub(crate) const LINE: usize = 1023;
/// The most bytes of a loaded file's own string (its name and arguments).
pub(crate) const NAME: usize = 255;

/// The text at an address, up to `most` bytes: what can be shown of it, and separately whether that is
/// every byte before an observed NUL, each of them printable. Only an exact string can authorize
/// anything; a longer or unreadable one is shown as far as it goes and marked not exact.
pub(crate) fn read(most: usize, mut byte_at: impl FnMut(usize) -> Option<u8>) -> (String, bool) {
    let mut text = String::new();
    let mut exact = true;
    for offset in 0..=most {
        match byte_at(offset) {
            Some(0) => return (text, exact),
            Some(b) if offset < most => {
                let printable = (0x20..0x7f).contains(&b);
                exact &= printable;
                text.push(if printable { b as char } else { '?' });
            }
            _ => break,
        }
    }
    (text, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(bytes: &[u8], most: usize) -> (String, bool) {
        read(most, |i| bytes.get(i).copied())
    }

    #[test]
    fn exact_up_to_the_limit_and_not_one_byte_past_it() {
        for most in [NAME, LINE] {
            let mut whole = alloc::vec![b'a'; most];
            whole.push(0);
            let (text, exact) = of(&whole, most);
            assert!(exact && text.len() == most);
            // One byte more than the limit before the NUL: shown as far as the limit, and not exact.
            let mut over = alloc::vec![b'a'; most + 1];
            over.push(0);
            let (text, exact) = of(&over, most);
            assert!(!exact && text.len() == most);
            // No NUL within reach (the memory ends): not exact.
            assert!(!of(&alloc::vec![b'a'; most - 1], most).1);
        }
        assert_eq!(of(b"\0", LINE), (String::new(), true));
        // A byte that cannot be shown is shown as a question mark and the text is not exact.
        assert_eq!(of(b"a\tb\0", LINE), (String::from("a?b"), false));
    }
}
