//! Tokens lexer reads from the source.

#![expect(dead_code, reason = "no lexer exists to read")]

use crate::{commentless_symbol::CommentlessSymbol, span::Span};

/// Holds a commentless symbol and where it is in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) symbol: CommentlessSymbol,
    pub(crate) span: Span,
}
