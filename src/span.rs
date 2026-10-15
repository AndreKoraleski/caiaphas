//! The span of something in the source grammar.

use std::ops::Range;

/// Where something is in the source, as byte offsets from its start.
///
/// `start` is the first byte, and `end` the byte after the last one, so an
/// empty span has `start == end`.
///
/// This means that the span of something can be denoted as [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Span {
    /// Returns the byte offsets the span covers, to index the source with.
    pub(crate) fn range(self) -> Range<usize> {
        self.start..self.end
    }

    /// Returns the span from the start of this one to the end of `other`.
    pub(crate) fn to(self, other: Self) -> Self {
        Self {
            start: self.start,
            end: other.end,
        }
    }
}
