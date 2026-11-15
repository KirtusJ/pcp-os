//! Validate only the status, framing fields and first markup of the fixed watch-page probe.
pub const DOCTYPE: &[u8] = b"<!DOCTYPE html>";

pub fn read(bytes: &[u8]) -> Result<Option<(&[u8], &[u8])>, &'static str> {
    let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") else {
        return Ok(None);
    };
    let mut lines = bytes[..end + 2].split_inclusive(|&b| b == b'\n');
    let status = lines
        .next()
        .and_then(|l| l.strip_suffix(b"\r\n"))
        .ok_or("invalid HTTP status line ending")?;
    if status.get(..13) != Some(b"HTTP/1.1 200 ")
        || !status[13..].iter().all(|c| (32..127).contains(c))
    {
        return Err("expected HTTP 200");
    }
    let (mut html, mut chunked, mut length) = (false, false, None);
    for field in lines {
        let field = field
            .strip_suffix(b"\r\n")
            .ok_or("invalid HTTPS field line ending")?;
        let at = field
            .iter()
            .position(|&b| b == b':')
            .ok_or("invalid HTTPS header")?;
        let name = &field[..at];
        if name.is_empty()
            || !name.iter().copied().all(super::token_byte)
            || !field[at + 1..]
                .iter()
                .all(|&b| b == b'\t' || (32..127).contains(&b))
        {
            return Err("invalid HTTPS field");
        }
        let value = field[at + 1..].trim_ascii();
        if name.eq_ignore_ascii_case(b"Content-Type") {
            if html
                || !value
                    .split(|&c| c == b';')
                    .next()
                    .is_some_and(|v| v.trim_ascii().eq_ignore_ascii_case(b"text/html"))
            {
                return Err("invalid HTML content type");
            }
            html = true;
        } else if name.eq_ignore_ascii_case(b"Transfer-Encoding") {
            if chunked || !value.eq_ignore_ascii_case(b"chunked") {
                return Err("unsupported HTTP transfer encoding");
            }
            chunked = true;
        } else if name.eq_ignore_ascii_case(b"Content-Length") {
            if length.is_some() || value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
                return Err("invalid content length");
            }
            length = Some(
                core::str::from_utf8(value)
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .ok_or("invalid content length")?,
            );
        } else if name.eq_ignore_ascii_case(b"Content-Encoding")
            && !value.eq_ignore_ascii_case(b"identity")
        {
            return Err("encoded HTTPS body");
        }
    }
    if !html || (chunked && length.is_some()) || length.is_some_and(|n| n < DOCTYPE.len()) {
        return Err("invalid HTTPS response framing");
    }
    let mut body = &bytes[end + 4..];
    if chunked {
        let Some(at) = body.windows(2).position(|v| v == b"\r\n") else {
            return Ok(None);
        };
        let digits = &body[..at];
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err("invalid first chunk size");
        }
        let size = core::str::from_utf8(digits)
            .ok()
            .and_then(|v| usize::from_str_radix(v, 16).ok())
            .ok_or("invalid first chunk size")?;
        if size < DOCTYPE.len() {
            return Err("short first HTML chunk");
        }
        body = &body[at + 2..];
    }
    let Some(prefix) = body.get(..DOCTYPE.len()) else {
        return Ok(None);
    };
    if !prefix.eq_ignore_ascii_case(DOCTYPE) {
        return Err("unexpected HTML prefix");
    }
    Ok(Some((status, prefix)))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    #[test]
    fn requires_the_complete_prefix_across_records() {
        for source in [b"HTTP/1.1 200 OK\r\nContent-type: text/html; charset=utf-8\r\nContent-Length: 15\r\n\r\n<!doctype html>".as_slice(),
                       b"HTTP/1.1 200 OK\r\nContent-type: text/html\r\nTransfer-Encoding: chunked\r\n\r\n100\r\n<!DOCTYPE html>"] {
            for cut in 0..source.len() {assert_eq!(read(&source[..cut]),Ok(None));}
            assert_eq!(read(source),Ok(Some((b"HTTP/1.1 200 OK".as_slice(), &source[source.len()-DOCTYPE.len()..]))));
        }
    }
    #[test]
    fn rejects_mismatched_or_ambiguous_responses() {
        for source in [b"HTTP/1.1 403 Denied\r\nContent-Type: text/html\r\n\r\n<!DOCTYPE html>".as_slice(),
            b"HTTP/1.1 200 OK\r\nX-Notice: Content-Type: text/html\r\n\r\n<!DOCTYPE html>",
            b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\nContent-Length: 100\r\n\r\n10\r\n<!DOCTYPE html>",
            b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nTransfer-Encoding: chunked\r\n\r\n1\r\n<!DOCTYPE html>"] {
            assert!(read(source).is_err());
        }
    }
    #[test]
    fn requires_valid_field_names_and_line_endings() {
        for field in [
            "Content-Length : 0",
            " Transfer-Encoding: gzip",
            ": 0",
            "X: a\u{000b}b",
            "X: a\nIgnored: b",
        ] {
            let response = std::format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{field}\r\n\r\n<!DOCTYPE html>"
            );
            assert!(read(response.as_bytes()).is_err());
        }
        let empty_reason = b"HTTP/1.1 200 \r\nContent-Type: text/html\r\n\r\n<!DOCTYPE html>";
        assert_eq!(
            read(empty_reason),
            Ok(Some((b"HTTP/1.1 200 ".as_slice(), DOCTYPE)))
        );
    }
}
