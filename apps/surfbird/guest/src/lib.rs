#![no_std]

//! An explicitly small HTTP/HTML fixture profile for a preconnected byte stream.
//! The hosted reader remains the reference for general HTML and URL semantics.
#[cfg(any(test, all(feature = "dns", target_os = "none")))]
pub mod dns_answer;
#[cfg(any(test, feature = "https-probe"))]
pub mod https_prefix;
mod href;
pub mod layout;
pub mod location;
#[cfg(any(test, feature = "https-probe"))]
pub mod watch_document;
pub const CAPACITY: usize = 2048;
pub const START: &str = "/harbour";

// RFC 9110 §5.6.2. Validate the name as received, before matching known fields.
fn token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

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
        if name.is_empty() || !name.bytes().all(token_byte) {
            return Err("HTTP header syntax");
        }
        if !value.bytes().all(|b| b == b'\t' || !b.is_ascii_control()) {
            return Err("HTTP header syntax");
        }
        let value = value.trim_matches([' ', '\t']);
        if name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("content-encoding")
        {
            return Err("unsupported encoding");
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err("invalid length");
            }
            length = Some(value.parse::<usize>().map_err(|_| "invalid length")?);
        }
        if name.eq_ignore_ascii_case("content-type") {
            if html || !value.eq_ignore_ascii_case("text/html") {
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
    target: href::Href,
}

impl Link<'_> {
    pub fn path(&self) -> &str {
        self.target.as_str()
    }
}
#[derive(Debug)]
pub struct Page<'a> {
    pub title: &'a str,
    pub paragraphs: [Paragraph<'a>; 12],
    pub paragraph_count: usize,
    pub links: [Link<'a>; 2],
    pub count: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Paragraph<'a> {
    pub id: Option<&'a str>,
    pub text: &'a str,
}

enum FragmentTarget {
    Top,
    Paragraph(usize),
    Missing,
}

impl Page<'_> {
    pub fn target_row(&self, fragment: Option<&str>, fallback: usize) -> usize {
        match self.fragment_target(fragment) {
            FragmentTarget::Top => 0,
            FragmentTarget::Paragraph(index) => index,
            FragmentTarget::Missing => fallback,
        }
    }

    fn fragment_target(&self, fragment: Option<&str>) -> FragmentTarget {
        let Some(id) = fragment.filter(|id| !id.is_empty()) else {
            return FragmentTarget::Top;
        };
        match self.paragraphs[..self.paragraph_count].iter().position(|p| p.id == Some(id)) {
            Some(index) => FragmentTarget::Paragraph(index),
            None if id.eq_ignore_ascii_case("top") => FragmentTarget::Top,
            None => FragmentTarget::Missing,
        }
    }
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
    let mut page = Page {
        title,
        paragraphs: [Paragraph::default(); 12],
        paragraph_count: 0,
        links: [Link::default(); 2],
        count: 0,
    };
    loop {
        rest = rest.trim_start();
        let id = if let Some(after) = rest.strip_prefix("<p>") {
            rest = after;
            None
        } else if let Some(after) = rest.strip_prefix("<p id=\"") {
            rest = after;
            let id = text(take(&mut rest, "\">")?)?;
            Some(id)
        } else {
            break;
        };
        let value = text(take(&mut rest, "</p>")?)?;
        *page
            .paragraphs
            .get_mut(page.paragraph_count)
            .ok_or("too many paragraphs")? = Paragraph { id, text: value };
        page.paragraph_count += 1;
    }
    if page.paragraph_count == 0 {
        return Err("expected paragraph");
    }
    while !rest.trim_start().is_empty() {
        rest = rest
            .trim_start()
            .strip_prefix("<a href=\"")
            .ok_or("unsupported markup")?;
        let target = href::Href::parse(take(&mut rest, "\">")?)?;
        let label = text(take(&mut rest, "</a>")?)?;
        *page.links.get_mut(page.count).ok_or("too many links")? = Link { label, target };
        page.count += 1;
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_all_fields_before_interpreting_framing() {
        const PREFIX: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 4\r\n";
        let check = |field: &str| {
            let mut head = [0; 256];
            head[..PREFIX.len()].copy_from_slice(PREFIX.as_bytes());
            let end = PREFIX.len() + field.len();
            head[PREFIX.len()..end].copy_from_slice(field.as_bytes());
            body_length(&head[..end])
        };
        for bad in [
            "Transfer-Encoding : chunked",
            "Transfer-Encoding\t: chunked",
            "X: value\r\n Transfer-Encoding: chunked",
            "X: value\r\n\tcontinued",
            ": empty",
            "Bad(Name): v",
            "Bad/Name: v",
            "Náme: v",
            "Bad\x7f: v",
            "X: one\nTransfer-Encoding: chunked",
            "X: one\rTransfer-Encoding: chunked",
            "X: bad\0value",
        ] {
            assert!(check(bad).is_err(), "accepted {bad:?}");
        }
        assert_eq!(check("!#$%&'*+-.^_`|~aZ09: \tvalid\tvalue "), Ok(4));
    }

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
        assert_eq!(p.links[1].label, "One");
        assert_eq!(p.links[1].path(), "/a");
    }
    #[test]
    fn named_paragraph_navigation_preserves_view_on_a_missing_fragment() {
        let page = parse(br##"<title>Pier</title><p>One</p><p>Two</p><p id="details">Three</p><a href="#details">Details</a>"##).unwrap();
        assert_eq!(page.links[0].path(), "#details");
        assert_eq!(page.target_row(Some("details"), 0), 2);
        assert_eq!(page.paragraphs[2].text, "Three");
        assert_eq!(page.target_row(Some("missing"), 1), 1);
        assert_eq!(page.target_row(Some("top"), 2), 0);
        assert_eq!(page.target_row(Some(""), 2), 0);
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
        for path in [
            "//other.test/a",
            "https://other.test",
            "/\r\nX",
            "/with space",
        ] {
            assert!(!location::supported_reference(path));
        }
    }
}
