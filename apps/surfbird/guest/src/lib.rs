#![no_std]

//! An explicitly small HTTP/HTML fixture profile for a preconnected byte stream.
//! The hosted reader remains the reference for general HTML and URL semantics.
pub const CAPACITY: usize = 2048;
pub const START: &str = "/harbour";

pub fn body_length(head: &[u8]) -> Result<usize, &'static str> {
    let mut fields = core::str::from_utf8(head)
        .map_err(|_| "HTTP header encoding")?
        .split("\r\n");
    if fields.next() != Some("HTTP/1.1 200 OK") {
        return Err("HTTP status");
    }
    let mut length = None;
    let mut html = false;
    for field in fields {
        let (name, value) = field.split_once(':').ok_or("HTTP header syntax")?;
        if name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("content-encoding")
        {
            return Err("unsupported encoding");
        }
        if name.eq_ignore_ascii_case("content-length") {
            let v = value.trim_matches([' ', '\t']);
            if length.is_some() || v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                return Err("invalid length");
            }
            length = Some(v.parse::<usize>().map_err(|_| "invalid length")?);
        }
        if name.eq_ignore_ascii_case("content-type") {
            if html || !value.trim().eq_ignore_ascii_case("text/html") {
                return Err("unsupported content type");
            }
            html = true;
        }
    }
    if !html {
        return Err("missing content type");
    }
    length.ok_or("missing length")
}

#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub struct Link<'a> {
    pub label: &'a str,
    pub path: &'a str,
}
#[derive(Debug)]
pub struct Page<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub links: [Link<'a>; 2],
    pub count: usize,
}

pub fn safe_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && path.len() <= 96
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_'))
}

fn take<'a>(input: &mut &'a str, end: &str) -> Result<&'a str, &'static str> {
    let (part, tail) = input.split_once(end).ok_or("incomplete HTML")?;
    *input = tail;
    Ok(part)
}

fn text(value: &str) -> Result<&str, &'static str> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|b| matches!(b, 0x20..=0x7e) && !matches!(b, b'<' | b'>' | b'&'))
    {
        return Err("unsupported page text");
    }
    Ok(value)
}

pub fn parse(body: &[u8]) -> Result<Page<'_>, &'static str> {
    let source = core::str::from_utf8(body).map_err(|_| "HTML encoding")?;
    let mut rest = source
        .trim()
        .strip_prefix("<title>")
        .ok_or("expected title")?;
    let title = text(take(&mut rest, "</title>")?)?;
    rest = rest
        .trim_start()
        .strip_prefix("<p>")
        .ok_or("expected paragraph")?;
    let paragraph = text(take(&mut rest, "</p>")?)?;
    let mut page = Page {
        title,
        text: paragraph,
        links: [Link::default(); 2],
        count: 0,
    };
    while !rest.trim_start().is_empty() {
        rest = rest
            .trim_start()
            .strip_prefix("<a href=\"")
            .ok_or("unsupported markup")?;
        let path = take(&mut rest, "\">")?;
        if !safe_path(path) {
            return Err("unsupported link target");
        }
        let label = text(take(&mut rest, "</a>")?)?;
        *page.links.get_mut(page.count).ok_or("too many links")? = Link { label, path };
        page.count += 1;
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_fixed_length_html_and_preserves_received_link_order() {
        assert_eq!(
            body_length(b"HTTP/1.1 200 OK\r\ncontent-type: text/html\r\nContent-Length: 42"),
            Ok(42)
        );
        let p = parse(
            br#"<title>Pier</title><p>Rising tide.</p><a href="/b">Two</a><a href="/a">One</a>"#,
        )
        .unwrap();
        assert_eq!(
            p.links[1],
            Link {
                label: "One",
                path: "/a"
            }
        );
    }
    #[test]
    fn rejects_ambiguous_framing_and_targets() {
        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 4";
        for bad in [
            "\r\nTransfer-Encoding: chunked",
            "\r\nContent-Length: 4",
            "\r\nContent-Encoding: gzip",
        ] {
            let mut bytes = [0; 128];
            let total = head.len() + bad.len();
            bytes[..head.len()].copy_from_slice(head.as_bytes());
            bytes[head.len()..total].copy_from_slice(bad.as_bytes());
            assert!(body_length(&bytes[..total]).is_err());
        }
        for path in ["//other.test/a", "https://other.test", "/\r\nX", "/../a"] {
            assert!(!safe_path(path));
        }
    }
}
