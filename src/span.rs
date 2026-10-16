//! Spans tell where something a parser read is in its input.

use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

/// Where something a parser read is in its input, with the text there.
///
/// Every span of a parse shares its input, so cloning one is cheap.
#[derive(Clone)]
pub struct Span {
    input: Arc<str>,
    start: usize,
    end: usize,
}

impl Span {
    pub(crate) fn new(input: Arc<str>, range: Range<usize>) -> Self {
        Self {
            input,
            start: range.start,
            end: range.end,
        }
    }

    /// Returns the text the span covers.
    pub fn as_str(&self) -> &str {
        &self.input[self.start..self.end]
    }

    /// Returns the byte offsets in the input that the span covers.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Returns the line the span starts on, counted from 1.
    pub fn line(&self) -> usize {
        line_and_column(&self.input, self.start).0
    }

    /// Returns the column the span starts at, counted in characters from 1.
    pub fn column(&self) -> usize {
        line_and_column(&self.input, self.start).1
    }
}

/// Returns the line and column of the byte `offset` in `input`, both counted
/// from 1, and the column in characters.
pub(crate) fn line_and_column(input: &str, offset: usize) -> (usize, usize) {
    let before = &input[..offset];
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let line = before.matches('\n').count() + 1;
    (line, before[line_start..].chars().count() + 1)
}

impl PartialEq for Span {
    fn eq(&self, other: &Self) -> bool {
        self.range() == other.range() && self.as_str() == other.as_str()
    }
}

impl Eq for Span {}

impl Hash for Span {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.range().hash(state);
        self.as_str().hash(state);
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {:?}", self.as_str(), self.range())
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(input: &str, range: Range<usize>) -> Span {
        Span::new(Arc::from(input), range)
    }

    #[test]
    fn covers_its_text() {
        let found = span("a = 'x';", 4..7);
        assert_eq!(found.as_str(), "'x'");
        assert_eq!(found.range(), 4..7);
        assert_eq!(found.to_string(), "'x'");
        assert_eq!(format!("{found:?}"), "\"'x'\" at 4..7");
    }

    #[test]
    fn finds_its_line_and_column_in_characters() {
        let found = span("first\nsé x", 10..11);
        assert_eq!(found.as_str(), "x");
        assert_eq!((found.line(), found.column()), (2, 4));
        let found = span("x", 0..1);
        assert_eq!((found.line(), found.column()), (1, 1));
    }

    #[test]
    fn equals_a_span_of_the_same_text_at_the_same_place() {
        assert_eq!(span("ab", 0..1), span("ac", 0..1));
        assert_ne!(span("ab", 0..1), span("cb", 0..1));
        assert_ne!(span("aa", 0..1), span("aa", 1..2));
    }

    #[test]
    fn hashes_what_it_compares() {
        let hash = |span: Span| {
            let mut hasher = std::hash::DefaultHasher::new();
            span.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(span("ab", 0..1)), hash(span("ac", 0..1)));
        assert_ne!(hash(span("ab", 0..1)), hash(span("cb", 0..1)));
    }
}
