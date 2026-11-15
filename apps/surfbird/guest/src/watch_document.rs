//! A bounded reader for the explicit HTML head of the one approved watch URL.
use crate::https_prefix::{Framing, DOCTYPE};

pub const URL: &str = "https://www.youtube.com/watch?v=M7lc1UVf-VE";
pub const LIMIT: usize = 2 * 1024 * 1024;
const TITLE_MAX: usize = 144;

pub struct Document {
    title: [u8; TITLE_MAX],
    length: usize,
    canonical: [u8; URL.len()],
}
impl Document {
    pub fn canonical(&self) -> &str {
        core::str::from_utf8(&self.canonical).unwrap()
    }
    pub fn title(&self) -> &str {
        core::str::from_utf8(&self.title[..self.length]).unwrap()
    }
}

// Decode only HTTP body bytes. Transfer-coding lines have an explicit small
// bound; refusing extensions/trailers is permissible in this narrow profile.
enum Phase {
    Size,
    SizeLf,
    Data(usize),
    DataCr,
    DataLf,
}
pub struct Body {
    framing: Framing,
    phase: Phase,
    digits: u8,
    size: usize,
    bytes: usize,
    attempts: u8,
}
impl Body {
    pub fn new(framing: Framing) -> Self {
        Self {
            framing,
            phase: Phase::Size,
            digits: 0,
            size: 0,
            bytes: 0,
            attempts: 0,
        }
    }
    pub fn decoded_bytes(&self) -> usize {
        self.bytes
    }
    pub fn feed(
        &mut self,
        mut input: &[u8],
        memory: &mut [u8],
    ) -> Result<Option<Document>, &'static str> {
        while !input.is_empty() {
            let count = match self.framing {
                Framing::Chunked => match self.phase {
                    Phase::Size => {
                        let c = input[0];
                        input = &input[1..];
                        if c == b'\r' && self.digits > 0 {
                            self.phase = Phase::SizeLf;
                        } else {
                            let digit = (c as char).to_digit(16).ok_or("unsupported chunk size")?;
                            if self.digits == 8 {
                                return Err("chunk size too long");
                            }
                            self.digits += 1;
                            self.size = self
                                .size
                                .checked_mul(16)
                                .and_then(|v| v.checked_add(digit as usize))
                                .ok_or("chunk size too large")?;
                        }
                        continue;
                    }
                    Phase::SizeLf => {
                        if input[0] != b'\n' {
                            return Err("invalid chunk line ending");
                        }
                        if self.size == 0 {
                            return Err("document metadata missing");
                        }
                        self.phase = Phase::Data(self.size);
                        self.digits = 0;
                        self.size = 0;
                        input = &input[1..];
                        continue;
                    }
                    Phase::Data(left) => {
                        let n = left.min(input.len());
                        self.phase = if n == left {
                            Phase::DataCr
                        } else {
                            Phase::Data(left - n)
                        };
                        n
                    }
                    Phase::DataCr | Phase::DataLf => {
                        let first = matches!(self.phase, Phase::DataCr);
                        if input[0] != if first { b'\r' } else { b'\n' } {
                            return Err("invalid chunk terminator");
                        }
                        self.phase = if first { Phase::DataLf } else { Phase::Size };
                        input = &input[1..];
                        continue;
                    }
                },
                Framing::Fixed(n) => {
                    if self.bytes >= n {
                        return Err("document metadata missing");
                    }
                    (n - self.bytes).min(input.len())
                }
                Framing::UntilClose => input.len(),
            };
            let prior = self.bytes;
            self.bytes = prior
                .checked_add(count)
                .filter(|&v| v <= memory.len() && v <= LIMIT)
                .ok_or("document prefix too large")?;
            memory[prior..self.bytes].copy_from_slice(&input[..count]);
            input = &input[count..];
            // Parsing only after a possible title end avoids repeated scans of large
            // inline scripts. Bound re-scans even when decoy text spans records.
            if memory[prior.saturating_sub(7)..self.bytes]
                .windows(8)
                .any(|s| s.eq_ignore_ascii_case(b"</title>"))
            {
                self.attempts += 1;
                if self.attempts > 8 {
                    return Err("too many title candidates");
                }
                if let Some(doc) = parse(&memory[..self.bytes])? {
                    return Ok(Some(doc));
                }
            }
        }
        Ok(None)
    }
}

fn tag_end(bytes: &[u8]) -> Option<usize> {
    let mut quote = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if quote != 0 {
            if b == quote {
                quote = 0;
            }
        } else if b == b'\'' || b == b'"' {
            quote = b;
        } else if b == b'>' {
            return Some(i);
        }
    }
    None
}

fn attribute<'a>(mut attrs: &'a [u8], key: &[u8]) -> Result<Option<&'a [u8]>, &'static str> {
    let mut found = None;
    loop {
        attrs = attrs.trim_ascii_start();
        if attrs.is_empty() || attrs == b"/" {
            return Ok(found);
        }
        let n = attrs
            .iter()
            .position(|b| b.is_ascii_whitespace() || *b == b'=' || *b == b'/')
            .unwrap_or(attrs.len());
        if n == 0 {
            return Err("invalid link attributes");
        }
        let name = &attrs[..n];
        attrs = attrs[n..].trim_ascii_start();
        let mut value = &attrs[..0];
        if attrs.first() == Some(&b'=') {
            attrs = attrs[1..].trim_ascii_start();
            let first = *attrs.first().ok_or("missing link attribute value")?;
            if first == b'\'' || first == b'"' {
                let end = attrs[1..]
                    .iter()
                    .position(|&b| b == first)
                    .ok_or("unterminated link attribute")?
                    + 1;
                value = &attrs[1..end];
                attrs = &attrs[end + 1..];
            } else {
                let end = attrs
                    .iter()
                    .position(u8::is_ascii_whitespace)
                    .unwrap_or(attrs.len());
                value = &attrs[..end];
                attrs = &attrs[end..];
            }
        }
        if name.eq_ignore_ascii_case(key) && found.replace(value).is_some() {
            return Err("duplicate link attribute");
        }
    }
}

fn title(raw: &[u8], canonical: [u8; URL.len()]) -> Result<Document, &'static str> {
    let mut doc = Document {
        title: [0; TITLE_MAX],
        length: 0,
        canonical,
    };
    let mut text = raw.trim_ascii();
    let mut space = false;
    while let Some((&b, rest)) = text.split_first() {
        text = rest;
        if b.is_ascii_whitespace() {
            space = true;
            continue;
        }
        if space && doc.length > 0 {
            *doc.title.get_mut(doc.length).ok_or("title too long")? = b' ';
            doc.length += 1;
        }
        space = false;
        let c = if b == b'&' {
            let end = text
                .iter()
                .take(9)
                .position(|&b| b == b';')
                .ok_or("unsupported title entity")?;
            let code = &text[..end];
            text = &text[end + 1..];
            match code {
                b"amp" => b'&',
                b"quot" => b'"',
                b"apos" => b'\'',
                b"lt" => b'<',
                b"gt" => b'>',
                _ => {
                    let s = core::str::from_utf8(code).map_err(|_| "invalid title entity")?;
                    if let Some(hex) = s.strip_prefix("#x").or_else(|| s.strip_prefix("#X")) {
                        u8::from_str_radix(hex, 16).ok()
                    } else {
                        s.strip_prefix('#').and_then(|d| d.parse().ok())
                    }
                    .ok_or("unsupported title entity")?
                }
            }
        } else {
            b
        };
        if !(32..127).contains(&c) {
            return Err("unsupported title character");
        }
        *doc.title.get_mut(doc.length).ok_or("title too long")? = c;
        doc.length += 1;
    }
    if doc.length == 0 {
        return Err("empty document title");
    }
    Ok(doc)
}

fn parse(mut html: &[u8]) -> Result<Option<Document>, &'static str> {
    if !html
        .get(..DOCTYPE.len())
        .is_some_and(|s| s.eq_ignore_ascii_case(DOCTYPE))
    {
        return Err("missing document doctype");
    }
    html = &html[DOCTYPE.len()..];
    let (mut root, mut head, mut canonical) = (false, false, None);
    loop {
        html = html.trim_ascii_start();
        if html.is_empty() {
            return Ok(None);
        }
        if html.starts_with(b"<!--") {
            let Some(i) = html[4..].windows(3).position(|s| s == b"-->") else {
                return Ok(None);
            };
            html = &html[4 + i + 3..];
            continue;
        }
        if html[0] != b'<' {
            return Err("text outside supported head");
        }
        let Some(end) = tag_end(&html[1..]).map(|n| n + 1) else {
            return Ok(None);
        };
        let tag = &html[1..end];
        html = &html[end + 1..];
        let split = tag
            .iter()
            .position(|c| c.is_ascii_whitespace() || *c == b'/')
            .unwrap_or(tag.len());
        let (name, attrs) = tag.split_at(split);
        if !root && name.eq_ignore_ascii_case(b"html") {
            root = true;
            continue;
        }
        if root && !head && name.eq_ignore_ascii_case(b"head") {
            head = true;
            continue;
        }
        if !head {
            return Err("explicit HTML head required");
        }
        if name.eq_ignore_ascii_case(b"meta") {
            continue;
        }
        if name.eq_ignore_ascii_case(b"link") {
            if attribute(attrs, b"rel")?.is_some_and(|v| v.eq_ignore_ascii_case(b"canonical")) {
                let href = attribute(attrs, b"href")?.ok_or("missing canonical URL")?;
                if canonical.is_some() || href != URL.as_bytes() {
                    return Err("unexpected canonical watch URL");
                }
                canonical = Some(href.try_into().map_err(|_| "unexpected URL length")?);
            }
            continue;
        }
        if name.eq_ignore_ascii_case(b"script")
            || name.eq_ignore_ascii_case(b"style")
            || name.eq_ignore_ascii_case(b"title")
        {
            let (closing, is_title): (&[u8], bool) = if name.eq_ignore_ascii_case(b"title") {
                (b"</title>", true)
            } else if name.eq_ignore_ascii_case(b"style") {
                (b"</style>", false)
            } else {
                (b"</script>", false)
            };
            let mut i = 0;
            loop {
                let Some(delta) = html[i..].iter().position(|&c| c == b'<') else {
                    return Ok(None);
                };
                i += delta;
                if html[i..]
                    .get(..closing.len())
                    .is_some_and(|s| s.eq_ignore_ascii_case(closing))
                {
                    break;
                }
                // A whitespace/slash closing tag would end raw text in a browser. Refuse
                // this alternate spelling instead of swallowing a different HTML context.
                let stem = &closing[..closing.len() - 1];
                if html[i..]
                    .get(..stem.len())
                    .is_some_and(|v| v.eq_ignore_ascii_case(stem))
                    && html
                        .get(i + stem.len())
                        .is_some_and(|c| c.is_ascii_whitespace() || *c == b'/')
                {
                    return Err("unsupported raw-text end tag");
                }
                if closing == b"</script>" && html[i..].starts_with(b"<!--") {
                    return Err("escaped script outside reader profile");
                }
                i += 1;
            }
            if is_title {
                return Ok(Some(title(
                    &html[..i],
                    canonical.ok_or("canonical watch link missing before title")?,
                )?));
            }
            html = &html[i + closing.len()..];
            continue;
        }
        return Err("unsupported markup before watch title");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::{format, vec, vec::Vec};

    fn head(before: &str, name: &str) -> Vec<u8> {
        format!("<!doctype html><html><head>{before}<link href='{URL}' rel=canonical><title>{name}</title></head>").into_bytes()
    }

    #[test]
    fn framing_and_tokens_across_all_byte_boundaries() {
        let document = head("<script>\"</title><title>script decoy\"</script><style>x</style><!-- <title>comment decoy</title> -->","A &amp; B &#x3a;  2");
        let mut wire = Vec::new();
        for fragment in document.chunks(11) {
            wire.extend(format!("{:x}\r\n", fragment.len()).as_bytes());
            wire.extend(fragment);
            wire.extend(b"\r\n");
        }
        wire.extend(b"0\r\n\r\n");
        for width in 1..=wire.len() {
            let mut mem = vec![0; document.len()];
            let mut body = Body::new(Framing::Chunked);
            let parsed = wire
                .chunks(width)
                .find_map(|input| body.feed(input, &mut mem).unwrap())
                .unwrap();
            assert_eq!(parsed.title(), "A & B : 2");
        }
        for framing in [Framing::UntilClose, Framing::Fixed(document.len())] {
            let mut mem = vec![0; document.len()];
            assert_eq!(
                Body::new(framing)
                    .feed(&document, &mut mem)
                    .unwrap()
                    .unwrap()
                    .title(),
                "A & B : 2"
            );
        }
    }

    #[test]
    fn refuses_untrusted_destinations_and_unsupported_grammar() {
        for before in [
            "<link rel=canonical href='https://other.example/'>",
            "<link rel=canonical rel=icon>",
            "<body>",
            "<template><title>decoy</title></template>",
            "<script><!-- <script>double escape</script> --> </script>",
        ] {
            assert!(parse(&head(before, "title")).is_err(), "{before}");
        }
        for value in ["", "&#27;A", "A\x1b", "&bogus;"] {
            assert!(parse(&head("", value)).is_err());
        }
        for d in [
            b"<!doctype html><html><head><title>unbound</title>".as_slice(),
            b"<!doctype html><html><head><link href='bad' rel='canonical'>",
        ] {
            assert!(parse(d).is_err());
        }
        let unfinished = b"<!doctype html><html><head><script>'</title>'";
        assert!(parse(unfinished).unwrap().is_none());
    }

    #[test]
    fn enforces_lengths_and_stops_on_faulty_chunks() {
        for bad in [
            b"3\r\nabcX\n".as_slice(),
            b"0\r\n",
            b"J\r\n",
            b"000000001\r\n",
            b"ff\rX",
            b"1;evil\r\n",
        ] {
            assert!(Body::new(Framing::Chunked)
                .feed(bad, &mut [0; 256])
                .is_err());
        }
        let full = head("", "document");
        assert!(Body::new(Framing::Fixed(20))
            .feed(&full, &mut [0; 256])
            .is_err());
        assert!(Body::new(Framing::UntilClose)
            .feed(&full, &mut [0; 30])
            .is_err());
    }
}
