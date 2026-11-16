//! The native reader uses the same visual rows for painting, scrolling and fragments.
use crate::{FragmentTarget, Page};

pub const VISIBLE_ROWS: usize = 9;
const TEXT_COLUMNS: usize = 58;
const LINK_COLUMNS: usize = 55; // Leave three cells for the keyboard shortcut.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Title,
    Paragraph(usize),
    Link(usize),
}

pub struct Line<'a> {
    pub source: Source,
    pub first: bool,
    pub text: &'a str,
}

fn wrap(text: &str, columns: usize) -> impl Iterator<Item = &str> {
    let mut remaining = Some(text.trim_matches(' '));
    core::iter::from_fn(move || {
        let text = remaining.take()?;
        let Some((boundary, _)) = text.char_indices().nth(columns) else {
            return Some(text);
        };
        let cut = if text[boundary..].starts_with(' ') {
            boundary
        } else {
            text[..boundary].rfind(' ').unwrap_or(boundary)
        };
        remaining = Some(text[cut..].trim_start_matches(' '));
        Some(text[..cut].trim_end_matches(' '))
    })
}

impl Page<'_> {
    pub fn lines(&self) -> impl Iterator<Item = Line<'_>> {
        let title = core::iter::once((Source::Title, self.title, TEXT_COLUMNS));
        let paragraphs = self.paragraphs[..self.paragraph_count]
            .iter()
            .enumerate()
            .map(|(i, p)| (Source::Paragraph(i), p.text, TEXT_COLUMNS));
        let links = self.links[..self.count]
            .iter()
            .enumerate()
            .map(|(i, link)| (Source::Link(i), link.label, LINK_COLUMNS));
        title
            .chain(paragraphs)
            .chain(links)
            .flat_map(|(source, text, columns)| {
                wrap(text, columns).enumerate().map(move |(i, text)| Line {
                    source,
                    first: i == 0,
                    text,
                })
            })
    }

    pub fn fragment_line(&self, fragment: Option<&str>, fallback: usize) -> usize {
        match self.fragment_target(fragment) {
            FragmentTarget::Top => 0,
            FragmentTarget::Missing => fallback,
            FragmentTarget::Paragraph(index) => self
                .lines()
                .position(|line| line.source == Source::Paragraph(index))
                .unwrap_or(fallback),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn wraps_at_spaces_and_breaks_long_words_without_losing_the_suffix() {
        let mut words = wrap("  Pier  Road and jetty  ", 9);
        assert_eq!(words.next(), Some("Pier"));
        assert_eq!(words.next(), Some("Road and"));
        assert_eq!(words.next(), Some("jetty"));
        assert_eq!(words.next(), None);
        let mut token = wrap("abcdefgh", 3);
        assert_eq!(token.next(), Some("abc"));
        assert_eq!(token.next(), Some("def"));
        assert_eq!(token.next(), Some("gh"));
        assert_eq!(token.next(), None);
        assert_eq!(wrap("   ", 58).count(), 1);
        assert_eq!(wrap("word  ", 4).count(), 1);
    }

    #[test]
    fn anchor_uses_its_first_visible_line_after_wrapped_predecessors() {
        let p = parse(br##"<title>A</title><p>12345678901234567890123456789012345678901234567890123456789</p><p id="top">Port Side</p><a href="#top">Alongside</a>"##).unwrap();
        assert_eq!(p.lines().count(), 5);
        assert_eq!(p.fragment_line(Some("top"), 0), 3);
        assert_eq!(p.fragment_line(Some("TOP"), 3), 0);
        assert_eq!(p.fragment_line(Some("missing"), 2), 2);
        assert_eq!(p.fragment_line(Some(""), 3), 0);
        assert_eq!(p.lines().nth(2).unwrap().text, "9");
        assert!(!p.lines().nth(2).unwrap().first);
    }
}
