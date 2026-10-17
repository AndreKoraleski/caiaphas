//! Syntax rules of ISO/IEC 14977 EBNF.

use crate::{
    annotation::{Expectation, Misspelling},
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
    /// Whether a `(* hide *)` right before the rule leaves it out of the
    /// generated types.
    pub(crate) hidden: bool,
    /// Whether a `(* greedy *)` comes right before the rule.
    pub(crate) greedy: bool,
    /// Whether a `(* recover *)` comes right before the rule.
    pub(crate) recovers: bool,
    /// Whether an `(* inline *)` comes right before the rule.
    pub(crate) inline: bool,
    /// The Rust type an `(* as T *)` right before the rule names, if any.
    pub(crate) converts_to: Option<String>,
    /// Each comment right before the rule of one word written almost as an
    /// annotation.
    pub(crate) misspellings: Vec<Misspelling>,
    /// The text of each comment right before the rule that is no annotation.
    pub(crate) comments: Vec<String>,
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
            return Err(parser.no_rule_name());
        };
        parser.bump();
        // A `-` inside a name reads as an exception, but one that starts `->`
        // defines a rule in other notations.
        let in_name = parser.peek().is_some_and(|token| {
            token.symbol == CommentlessSymbol::Symbol(Symbol::Except)
                && !parser.lexer.source()[token.span.end..].starts_with('>')
        });
        let expected = if in_name {
            Expected::DefiningSymbolNotExcept
        } else {
            Expected::DefiningSymbol
        };
        parser.expect(Symbol::Defining, expected)?;
        let definitions_list = DefinitionsList::parse(parser)?;
        if parser.peek_symbol() != Some(Symbol::Terminator) {
            return Err(parser.unfinished());
        }
        let end =
            parser.expect(Symbol::Terminator, Expected::TerminatorSymbol)?;
        Ok(Self {
            meta_identifier: name,
            definitions_list,
            expectations: Vec::new(),
            hidden: false,
            greedy: false,
            recovers: false,
            inline: false,
            converts_to: None,
            misspellings: Vec::new(),
            comments: Vec::new(),
            span: start.to(end),
        })
    }
}
