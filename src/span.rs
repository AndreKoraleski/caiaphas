//! The span of something in the source grammar.

#![cfg_attr(not(test), expect(dead_code, reason = "no lexer exists to read"))]

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
