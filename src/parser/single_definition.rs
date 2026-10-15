//! Single definitions of ISO/IEC 14977 EBNF.

use crate::{
    lexer::Symbol,
    parser::{ParseError, Parser, syntactic_term::SyntacticTerm},
    span::Span,
};

/// A single definition is one or more syntactic terms joined by `,`. They
/// match one after another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleDefinition {
    pub(crate) syntactic_terms: Vec<SyntacticTerm>,
    pub(crate) span: Span,
}

impl SingleDefinition {
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let first = SyntacticTerm::parse(parser)?;
        let mut span = first.span;
        let mut syntactic_terms = vec![first];
        while parser.eat(Symbol::Concatenate).is_some() {
            let term = SyntacticTerm::parse(parser)?;
            span = span.to(term.span);
            syntactic_terms.push(term);
        }
        Ok(Self {
            syntactic_terms,
            span,
        })
    }
}
