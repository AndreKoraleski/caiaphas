//! Syntactic factors of ISO/IEC 14977 EBNF.

use crate::{
    lexer::{CommentlessSymbol, Integer, Symbol},
    parser::{
        Expected, ParseError, Parser, syntactic_primary::SyntacticPrimary,
    },
    span::Span,
};

/// A syntactic factor is a syntactic primary that may be repeated a fixed
/// number of times, such as `3 * 'a'`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SyntacticFactor {
    /// How many times the primary repeats, if written before a `*`.
    pub(crate) integer: Option<Integer>,
    pub(crate) syntactic_primary: SyntacticPrimary,
    pub(crate) span: Span,
}

impl SyntacticFactor {
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let integer = parser.peek().and_then(|token| match token.symbol {
            CommentlessSymbol::Integer(integer) => Some((integer, token.span)),
            _ => None,
        });
        if integer.is_some() {
            parser.bump();
            parser.expect(Symbol::Repetition, Expected::RepetitionSymbol)?;
            // A primary cannot be a count, as one more `* x` would make it.
            if parser.peek().is_some_and(|token| {
                matches!(token.symbol, CommentlessSymbol::Integer(_))
            }) {
                return Err(parser.unexpected(Expected::PrimaryNotCount));
            }
        }
        let primary = SyntacticPrimary::parse(parser)?;
        let span =
            integer.map_or(primary.span, |(_, start)| start.to(primary.span));
        Ok(Self {
            integer: integer.map(|(integer, _)| integer),
            syntactic_primary: primary,
            span,
        })
    }
}
