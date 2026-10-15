//! Tokens the lexer reads from the source.

use crate::{lexer::commentless_symbol::CommentlessSymbol, span::Span};

/// Holds a commentless symbol and where it is in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) symbol: CommentlessSymbol,
    pub(crate) span: Span,
}
