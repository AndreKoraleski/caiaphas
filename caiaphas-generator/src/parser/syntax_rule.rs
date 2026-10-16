//! Syntax rules of ISO/IEC 14977 EBNF.

use crate::{
    annotation::Expectation,
    lexer::{CommentlessSymbol, MetaIdentifier, Symbol},
    parser::{
        Expected, ParseError, Parser, definitions_list::DefinitionsList,
    },
    span::Span,
};

/// A syntax rule defines a meta identifier as a definitions list, such as
/// `digit = '0' | '1';`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SyntaxRule {
    pub(crate) meta_identifier: MetaIdentifier,
    pub(crate) definitions_list: DefinitionsList,
    /// The findings the comments right before the rule mark as intended.
    pub(crate) expectations: Vec<Expectation>,
    /// Whether a hide right before the rule leaves it out of the generated
    /// types.
    pub(crate) hidden: bool,
    /// Whether a first match right before the rule makes its choices take
    /// their first alternative that matches.
    pub(crate) first_match: bool,
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
        let definitions_list = DefinitionsList::parse(parser)?;
        let end =
            parser.expect(Symbol::Terminator, Expected::TerminatorSymbol)?;
        Ok(Self {
            meta_identifier: name,
            definitions_list,
            expectations: Vec::new(),
            hidden: false,
            first_match: false,
            span: start.to(end),
        })
    }
}
