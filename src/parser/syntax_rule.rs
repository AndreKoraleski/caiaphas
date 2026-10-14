//! Syntax rules of ISO/IEC 14977 EBNF.

use crate::{
    lexer::{CommentlessSymbol, MetaIdentifier, Symbol},
    parser::{
        Expected, ParseError, Parser, definitions_list::DefinitionsList,
    },
    span::Span,
};

/// A syntax rule defines a meta identifier as a definitions list, such as
/// `digit = '0' | '1';`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyntaxRule {
    pub(crate) name: MetaIdentifier,
    pub(crate) definitions: DefinitionsList,
    /// From the meta identifier to the terminator, both included.
    pub(crate) span: Span,
}

impl SyntaxRule {
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let Some((name, start)) =
            parser.peek().and_then(|token| match &token.symbol {
                CommentlessSymbol::MetaIdentifier(name) => {
                    Some((name.clone(), token.span))
                }
                _ => None,
            })
        else {
            return Err(parser.unexpected(Expected::MetaIdentifier));
        };
        parser.bump();
        parser.expect(Symbol::Defining, Expected::DefiningSymbol)?;
        let definitions = DefinitionsList::parse(parser)?;
        let end =
            parser.expect(Symbol::Terminator, Expected::TerminatorSymbol)?;
        Ok(Self {
            name,
            definitions,
            span: start.to(end),
        })
    }
}
