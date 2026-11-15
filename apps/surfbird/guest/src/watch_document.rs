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

struct StartTag<'a> {
    name: &'a [u8],
    rel: Option<&'a [u8]>,
    href: Option<&'a [u8]>,
    rest: &'a [u8],
}

// A quote opens an attribute value only after a name and '='. In particular,
// a stray quote in an unquoted attribute must never hide a tag boundary.
fn start_tag(input: &[u8]) -> Result<Option<StartTag<'_>>, &'static str> {
    let mut rest = &input[1..];
    let name_end = rest.iter().position(|b| !b.is_ascii_alphabetic());
    let Some(end) = name_end else { return Ok(None) };
    let name = &rest[..end];
    rest = &rest[end..];
    let (mut rel, mut href) = (None, None);
    loop {
        let had_space = rest.first().is_some_and(u8::is_ascii_whitespace);
        rest = rest.trim_ascii_start();
        if let Some(tail) = rest.strip_prefix(b"/>").or_else(|| rest.strip_prefix(b">")) {
            return Ok(Some(StartTag {
                name,
                rel,
                href,
                rest: tail,
            }));
        }
        if rest.is_empty() || rest == b"/" {
            return Ok(None);
        }
        if !had_space {
            return Err("invalid start tag");
        }
        let end = rest.iter().position(|b| {
            b.is_ascii_whitespace() || matches!(b, b'=' | b'/' | b'>' | b'<' | b'\'' | b'"' | 0)
        });
        let Some(end) = end else { return Ok(None) };
        if end == 0 {
            return Err("invalid attribute name");
        }
        let attr = &rest[..end];
        rest = &rest[end..];
        let mut value = &rest[..0];
        if let Some(after) = rest.trim_ascii_start().strip_prefix(b"=") {
            rest = after.trim_ascii_start();
            let Some(&first) = rest.first() else {
                return Ok(None);
            };
            if matches!(first, b'\'' | b'"') {
                let Some(end) = rest[1..].iter().position(|&b| b == first) else {
                    return Ok(None);
                };
                value = &rest[1..1 + end];
                rest = &rest[2 + end..];
            } else {
                let end = rest.iter().position(|b| {
                    b.is_ascii_whitespace() || matches!(b, b'>' | b'<' | b'\'' | b'"' | 0)
                });
                let Some(end) = end else { return Ok(None) };
                if end == 0 || matches!(rest[end], b'<' | b'\'' | b'"' | 0) {
                    return Err("invalid unquoted attribute");
                }
                value = &rest[..end];
                rest = &rest[end..];
            }
        }
        if name.eq_ignore_ascii_case(b"link") {
            if attr.eq_ignore_ascii_case(b"rel") {
                if value.contains(&b'&') || rel.replace(value).is_some() {
                    return Err("ambiguous link relation");
                }
            } else if attr.eq_ignore_ascii_case(b"href") && href.replace(value).is_some() {
                return Err("duplicate link address");
            }
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
                    let (digits, radix) =
                        if let Some(hex) = s.strip_prefix("#x").or_else(|| s.strip_prefix("#X")) {
                            (hex, 16)
                        } else {
                            (s.strip_prefix('#').ok_or("unsupported title entity")?, 10)
                        };
                    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err("invalid numeric title entity");
                    }
                    u8::from_str_radix(digits, radix).map_err(|_| "unsupported title entity")?
                }
            }
        } else {
            b
        };
        if c.is_ascii_whitespace() {
            space = true;
            continue;
        }
        if !(32..127).contains(&c) {
            return Err("unsupported title character");
        }
        if space && doc.length > 0 {
            *doc.title.get_mut(doc.length).ok_or("title too long")? = b' ';
            doc.length += 1;
        }
        space = false;
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
            let comment = &html[4..];
            if comment.starts_with(b">") || comment.starts_with(b"->") {
                return Err("abrupt HTML comment ending");
            }
            let Some(end) = comment.windows(2).position(|s| s == b"--") else {
                return Ok(None);
            };
            let Some(&next) = comment.get(end + 2) else {
                return Ok(None);
            };
            // Support ordinary comments only. In particular, do not scan past
            // the alternate --!> terminator into script/style raw text.
            if next != b'>' {
                return Err("unsupported HTML comment");
            }
            html = &comment[end + 3..];
            continue;
        }
        if html[0] != b'<' {
            return Err("text outside supported head");
        }
        let Some(tag) = start_tag(html)? else {
            return Ok(None);
        };
        let name = tag.name;
        html = tag.rest;
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
            if tag.rel.is_some_and(|v| {
                v.split(u8::is_ascii_whitespace)
                    .any(|token| token.eq_ignore_ascii_case(b"canonical"))
            }) {
                let href = tag.href.ok_or("missing canonical URL")?;
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
        for value in [
            "", "&#32;", "&#27;A", "A\x1b", "&bogus;", "&#+65;", "&#x+41;",
        ] {
            assert!(parse(&head("", value)).is_err());
        }
        assert_eq!(
            parse(&head("", "&#32;A &#9; &#x20;B&#10;"))
                .unwrap()
                .unwrap()
                .title(),
            "A B"
        );
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
    fn ambiguous_comments_and_tag_boundaries_cannot_promote_script_text() {
        for prefix in [
            "<!-- harmless --!><script type=text/plain> -->",
            "<!--><script type=text/plain> -->",
            "<!---><script type=text/plain> -->",
            "<meta data-x=b'><script type=text/plain> '>",
            "<meta x'=y><script type=text/plain> '>",
        ] {
            let input = format!("<!doctype html><html><head>{prefix}<link rel=canonical href='{URL}'><title>Script decoy</title></script><title>Actual title</title>");
            for width in 1..=input.len() {
                let mut body = Body::new(Framing::UntilClose);
                let mut memory = vec![0; input.len()];
                let result = input.as_bytes().chunks(width).try_for_each(|chunk| {
                    body.feed(chunk, &mut memory)
                        .map(|doc| assert!(doc.is_none()))
                });
                assert!(result.is_err(), "{prefix}");
            }
        }
        let supported = head(
            "<meta data-x=\"a>b'c\" disabled><!----><!-- <title>decoy</title> -->",
            "actual",
        );
        assert_eq!(parse(&supported).unwrap().unwrap().title(), "actual");
        for before in [
            "<link rel='alternate canonical' href='https://elsewhere.example/'>",
            "<link rel='canon&#105;cal' href='https://elsewhere.example/'>",
        ] {
            assert!(parse(&head(before, "title")).is_err());
        }
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
