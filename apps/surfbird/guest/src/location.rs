//! The fixed-origin guest stores bounded ASCII URL references without a heap.
const LIMIT: usize = 96;

#[derive(Clone, Copy)]
pub struct Location {
    bytes: [u8; LIMIT],
    len: usize,
}

fn before(s: &str, delimiter: char) -> &str {
    s.split_once(delimiter).map_or(s, |(left, _)| left)
}

pub fn supported_reference(s: &str) -> bool {
    s.len() <= LIMIT
        && !s.starts_with("//")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.~?&=+;,#".contains(&b))
}

impl Location {
    fn from_ascii(s: &str) -> Result<Self, &'static str> {
        let mut result = Self {
            bytes: [0; LIMIT],
            len: s.len(),
        };
        result
            .bytes
            .get_mut(..s.len())
            .ok_or("link target too long")?
            .copy_from_slice(s.as_bytes());
        Ok(result)
    }

    pub fn start() -> Self {
        Self::from_ascii(crate::START).unwrap()
    }

    pub fn address(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }

    pub fn request_target(&self) -> &str {
        before(self.address(), '#')
    }

    pub fn fragment(&self) -> Option<&str> {
        self.address().split_once('#').map(|(_, fragment)| fragment)
    }

    pub fn follow(&self, href: &str) -> Result<Self, &'static str> {
        if !supported_reference(href) {
            return Err("unsupported link target");
        }
        let base = self.request_target();
        let base_path = before(base, '?');
        let path = before(before(href, '#'), '?');
        let mut merged = [0; LIMIT * 2];
        let prefix = if path.is_empty() {
            if href.starts_with('?') {
                base_path
            } else {
                base
            }
        } else if path.starts_with('/') {
            ""
        } else {
            &base_path[..=base_path.rfind('/').unwrap()]
        };
        merged[..prefix.len()].copy_from_slice(prefix.as_bytes());
        let end = prefix.len() + href.len();
        merged[prefix.len()..end].copy_from_slice(href.as_bytes());
        let merged = core::str::from_utf8(&merged[..end]).unwrap();
        if path.is_empty() {
            return Self::from_ascii(merged);
        }

        // Remove dot segments only from the path. Empty segments, including a
        // trailing slash, are significant (RFC 3986 sections 5.2.4 and 5.4).
        let full_path = before(before(merged, '#'), '?');
        let mut normalized = [0; LIMIT * 2];
        let mut used = 0;
        let mut segments = full_path[1..].split('/').peekable();
        while let Some(segment) = segments.next() {
            if segment == ".." {
                used = normalized[..used]
                    .iter()
                    .rposition(|&b| b == b'/')
                    .unwrap_or(0);
            }
            if segment == "." || segment == ".." {
                if segments.peek().is_some() {
                    continue;
                }
                normalized[used] = b'/';
                used += 1;
            } else {
                normalized[used] = b'/';
                used += 1;
                normalized[used..used + segment.len()].copy_from_slice(segment.as_bytes());
                used += segment.len();
            }
        }
        let suffix = &merged[full_path.len()..];
        normalized[used..used + suffix.len()].copy_from_slice(suffix.as_bytes());
        let address = core::str::from_utf8(&normalized[..used + suffix.len()]).unwrap();
        if address.starts_with("//") {
            return Err("unsupported link target");
        }
        Self::from_ascii(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_origin_reference_examples() {
        let base = Location::start().follow("/b/c/d;p?q").unwrap();
        // Same-origin cases from RFC 3986 section 5.4, plus empty components.
        for (href, address) in [
            ("g", "/b/c/g"),
            ("./g", "/b/c/g"),
            ("g/", "/b/c/g/"),
            ("/g", "/g"),
            ("?y", "/b/c/d;p?y"),
            ("g?y", "/b/c/g?y"),
            ("#s", "/b/c/d;p?q#s"),
            ("g#s", "/b/c/g#s"),
            ("g?y#s", "/b/c/g?y#s"),
            (";x", "/b/c/;x"),
            ("g;x", "/b/c/g;x"),
            ("g;x?y#s", "/b/c/g;x?y#s"),
            ("", "/b/c/d;p?q"),
            (".", "/b/c/"),
            ("./", "/b/c/"),
            ("..", "/b/"),
            ("../", "/b/"),
            ("../g", "/b/g"),
            ("../..", "/"),
            ("../../", "/"),
            ("../../g", "/g"),
            ("../../../g", "/g"),
            ("/./g", "/g"),
            ("/../g", "/g"),
            ("./../g", "/b/g"),
            ("g/./h", "/b/c/g/h"),
            ("g/../h", "/b/c/h"),
            ("g?y/../x", "/b/c/g?y/../x"),
            ("g#s/./x", "/b/c/g#s/./x"),
            ("?", "/b/c/d;p?"),
            ("#", "/b/c/d;p?q#"),
            ("g//h", "/b/c/g//h"),
            ("?x=1&y=2", "/b/c/d;p?x=1&y=2"),
        ] {
            let next = base.follow(href).unwrap();
            assert_eq!(next.address(), address, "{href}");
            assert_eq!(next.request_target(), before(address, '#'));
        }
        let with_fragment = base.follow("#old").unwrap();
        assert_eq!(
            with_fragment.follow("#new").unwrap().address(),
            "/b/c/d;p?q#new"
        );
        assert_eq!(with_fragment.follow("").unwrap().address(), "/b/c/d;p?q");
        for rejected in [
            "//other",
            "https://other.test",
            "x y",
            "\\other",
            "/.//other",
            "%2e",
        ] {
            assert!(base.follow(rejected).is_err(), "{rejected}");
        }
    }

    #[test]
    fn resolved_length_is_bounded_after_normalization() {
        let path = [b'x'; LIMIT - 1];
        let name = core::str::from_utf8(&path).unwrap();
        let full = Location::start().follow(name).unwrap();
        assert_eq!(full.address().len(), LIMIT);
        assert!(full.follow("#").is_err());
        let base = Location::start().follow("/directory/start").unwrap();
        assert!(base.follow(name).is_err());
        assert_eq!(base.follow("../x").unwrap().request_target(), "/x");
    }
}
