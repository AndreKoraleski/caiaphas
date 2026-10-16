//! Definitions lists of ISO/IEC 14977 EBNF.

use crate::{
    lexer::Symbol,
    parser::{ParseError, Parser, single_definition::SingleDefinition},
    span::Span,
};

/// A definitions list is one or more single definitions separated by `|`, `/`
/// or `!`. Each is an alternative to the others.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DefinitionsList {
    pub(crate) single_definitions: Vec<SingleDefinition>,
    pub(crate) span: Span,
}

impl DefinitionsList {
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let first = SingleDefinition::parse(parser)?;
        let mut span = first.span;
        let mut single_definitions = vec![first];
        while parser.eat(Symbol::DefinitionSeparator).is_some() {
            let definition = SingleDefinition::parse(parser)?;
            span = span.to(definition.span);
            single_definitions.push(definition);
        }
        Ok(Self {
            single_definitions,
            span,
        })
    }
}
