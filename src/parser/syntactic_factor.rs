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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyntacticFactor {
    /// How many times the primary repeats, if written before a `*`.
    pub(crate) repetitions: Option<Integer>,
    pub(crate) primary: SyntacticPrimary,
    pub(crate) span: Span,
}

impl SyntacticFactor {
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let count = parser.peek().and_then(|token| match token.symbol {
            CommentlessSymbol::Integer(integer) => Some((integer, token.span)),
            _ => None,
        });
        if count.is_some() {
            parser.bump();
            parser.expect(Symbol::Repetition, Expected::RepetitionSymbol)?;
        }
        let primary = SyntacticPrimary::parse(parser)?;
        let span =
            count.map_or(primary.span, |(_, start)| start.to(primary.span));
        Ok(Self {
            repetitions: count.map(|(integer, _)| integer),
            primary,
            span,
        })
    }
}
