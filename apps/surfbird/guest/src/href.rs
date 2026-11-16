//! Decode the quoted HTML attribute once, before applying the guest URL profile.
use crate::location::{supported_reference, LIMIT};

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) struct Href {
    bytes: [u8; LIMIT],
    len: usize,
}

impl Default for Href {
    fn default() -> Self {
        Self {
            bytes: [0; LIMIT],
            len: 0,
        }
    }
}

impl Href {
    pub(crate) fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }

    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        if !value.is_ascii() {
            return Err("unsupported link target");
        }
        let mut result = Self::default();
        let mut rest = value.as_bytes();
        while let Some((&byte, tail)) = rest.split_first() {
            rest = tail;
            if byte == b'&' {
                result.push_reference(&mut rest)?;
            } else {
                result.push(core::slice::from_ref(&byte))?;
            }
        }
        if !supported_reference(result.as_str()) {
            return Err("unsupported link target");
        }
        Ok(result)
    }

    fn push(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let end = self.len + bytes.len();
        self.bytes
            .get_mut(self.len..end)
            .ok_or("link target too long")?
            .copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }
}

// The HTML named references whose complete expansion fits the URL alphabet.
fn named(name: &[u8]) -> Option<&'static [u8]> {
    Some(match name {
        b"AMP" | b"amp" => b"&",
        b"comma" => b",",
        b"equals" => b"=",
        b"fjlig" => b"fj",
        b"lowbar" | b"UnderBar" => b"_",
        b"num" => b"#",
        b"period" => b".",
        b"plus" => b"+",
        b"quest" => b"?",
        b"semi" => b";",
        b"sol" => b"/",
        _ => return None,
    })
}

// Other legacy HTML names can also consume a missing semicolon. Their
// expansions are outside the profile. Bare query names like &flag stay literal.
const OTHER_LEGACY: &str = "AElig Aacute Acirc Agrave Aring Atilde Auml COPY Ccedil ETH Eacute Ecirc Egrave Euml GT Iacute Icirc Igrave Iuml LT Ntilde Oacute Ocirc Ograve Oslash Otilde Ouml QUOT REG THORN Uacute Ucirc Ugrave Uuml Yacute aacute acirc acute aelig agrave aring atilde auml brvbar ccedil cedil cent copy curren deg divide eacute ecirc egrave eth euml frac12 frac14 frac34 gt iacute icirc iexcl igrave iquest iuml laquo lt macr micro middot nbsp not ntilde oacute ocirc ograve ordf ordm oslash otilde ouml para plusmn pound quot raquo reg sect shy sup1 sup2 sup3 szlig thorn times uacute ucirc ugrave uml uuml yacute yen yuml";

impl Href {
    fn push_reference(&mut self, rest: &mut &[u8]) -> Result<(), &'static str> {
        if let Some(number) = rest.strip_prefix(b"#") {
            let (digits, radix) = match number.first() {
                Some(b'x' | b'X') => (&number[1..], 16),
                _ => (number, 10),
            };
            let end = digits
                .iter()
                .take_while(|b| {
                    if radix == 16 {
                        b.is_ascii_hexdigit()
                    } else {
                        b.is_ascii_digit()
                    }
                })
                .count();
            if end == 0 {
                return Err("invalid numeric link reference");
            }
            let code = u8::from_str_radix(core::str::from_utf8(&digits[..end]).unwrap(), radix)
                .map_err(|_| "unsupported link character")?;
            // Only ASCII can enter as_str(). URL punctuation is checked after decoding.
            if !(32..=126).contains(&code) {
                return Err("unsupported link character");
            }
            *rest = digits[end..].strip_prefix(b";").unwrap_or(&digits[end..]);
            return self.push(&[code]);
        }
        let end = rest
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric())
            .count();
        let (name, tail) = rest.split_at(end);
        if !name.is_empty() {
            if let Some(after) = tail.strip_prefix(b";") {
                *rest = after;
                return self.push(named(name).ok_or("unsupported named link reference")?);
            }
            if !tail.starts_with(b"=") {
                if matches!(name, b"amp" | b"AMP") {
                    *rest = tail;
                } else if OTHER_LEGACY
                    .split_ascii_whitespace()
                    .any(|entry| entry.as_bytes() == name)
                {
                    return Err("unsupported named link reference");
                }
            }
        }
        self.push(b"&")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{location::Location, parse};

    #[test]
    fn html_attribute_is_decoded_once_before_url_resolution() {
        let page = parse(br#"<title>Pier</title><p>Tide</p><a href="./jetty?view=2&amp;deck=upper&#35;berth">Jetty</a>"#).unwrap();
        let target = Location::start().follow(page.links[0].path()).unwrap();
        assert_eq!(target.address(), "/jetty?view=2&deck=upper#berth");
        assert_eq!(target.request_target(), "/jetty?view=2&deck=upper");
        assert_eq!(Href::parse("&amp;amp;").unwrap().as_str(), "&amp;");
        for (raw, decoded) in [
            ("?a=1&flag&y=2&amp=3&notit=4", "?a=1&flag&y=2&amp=3&notit=4"),
            ("&AMP;v&#x3d;1&#X26w=2", "&v=1&w=2"),
            ("&fjlig;&sol;&comma;&lowbar;", "fj/,_"),
            ("&amp&quest;", "&?"),
        ] {
            assert_eq!(Href::parse(raw).unwrap().as_str(), decoded, "{raw}");
        }
    }

    #[test]
    fn validates_the_decoded_authority_characters_and_length() {
        for raw in [
            "/&#47;other",
            "&sol;/other",
            "&#10;",
            "&#x80;",
            "&#x;",
            "&copy",
            "&copy;",
            "&unknown;",
            "&#256;",
            "/&#32;space",
        ] {
            assert!(Href::parse(raw).is_err(), "{raw}");
        }
        let encoded = "&amp;".repeat(LIMIT);
        assert_eq!(Href::parse(&encoded).unwrap().as_str().len(), LIMIT);
        assert!(Href::parse(&(encoded + "x")).is_err());
        assert!(Href::parse(&("a".repeat(LIMIT - 1) + "&fjlig;")).is_err());
    }
}
