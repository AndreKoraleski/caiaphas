//! Syntactic terms and syntactic exceptions of ISO/IEC 14977 EBNF.

use crate::{
    lexer::Symbol,
    parser::{ParseError, Parser, syntactic_factor::SyntacticFactor},
    span::Span,
};

/// A syntactic term is a syntactic factor, minus what a syntactic exception
/// matches if one follows a `-`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyntacticTerm {
    pub(crate) factor: SyntacticFactor,
    /// The standard restricts a syntactic exception to factors that could be
    /// written without meta identifiers.
    pub(crate) exception: Option<SyntacticFactor>,
    pub(crate) span: Span,
}

impl SyntacticTerm {
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let factor = SyntacticFactor::parse(parser)?;
        let exception = if parser.eat(Symbol::Except).is_some() {
            Some(SyntacticFactor::parse(parser)?)
        } else {
            None
        };
        let span = exception
            .as_ref()
            .map_or(factor.span, |exception| factor.span.to(exception.span));
        Ok(Self {
            factor,
            exception,
            span,
        })
    }
}
