//! Syntactic primaries of ISO/IEC 14977 EBNF.

use crate::{
    lexer::{
        CommentlessSymbol, MetaIdentifier, SpecialSequence, Symbol,
        TerminalString,
    },
    parser::{
        Expected, ParseError, Parser, definitions_list::DefinitionsList,
    },
    span::Span,
};

/// A syntactic primary is the smallest part of a definition, such as a
/// terminal string or a group in brackets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyntacticPrimary {
    pub(crate) kind: SyntacticPrimaryKind,
    /// An empty sequence has an empty span where it would be.
    pub(crate) span: Span,
}

/// What a [`SyntacticPrimary`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyntacticPrimaryKind {
    /// Matches the definitions between `[` and `]` once or not at all.
    OptionalSequence(DefinitionsList),
    /// Matches the definitions between `{` and `}` any number of times.
    RepeatedSequence(DefinitionsList),
    /// Matches the definitions between `(` and `)` once.
    GroupedSequence(DefinitionsList),
    MetaIdentifier(MetaIdentifier),
    TerminalString(TerminalString),
    SpecialSequence(SpecialSequence),
    /// Matches without reading anything.
    EmptySequence,
}

impl SyntacticPrimary {
    /// Reads a syntactic primary, or an empty sequence if the next token does
    /// not start one.
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let Some(token) = parser.peek() else {
            return Ok(Self::empty(parser));
        };
        let span = token.span;
        let kind = match &token.symbol {
            CommentlessSymbol::MetaIdentifier(name) => {
                SyntacticPrimaryKind::MetaIdentifier(name.clone())
            }
            CommentlessSymbol::TerminalString(string) => {
                SyntacticPrimaryKind::TerminalString(string.clone())
            }
            CommentlessSymbol::SpecialSequence(sequence) => {
                SyntacticPrimaryKind::SpecialSequence(sequence.clone())
            }
            CommentlessSymbol::Symbol(Symbol::StartOption) => {
                return Self::parse_bracketed(
                    parser,
                    span,
                    Symbol::EndOption,
                    Expected::EndOptionSymbol,
                    SyntacticPrimaryKind::OptionalSequence,
                );
            }
            CommentlessSymbol::Symbol(Symbol::StartRepeat) => {
                return Self::parse_bracketed(
                    parser,
                    span,
                    Symbol::EndRepeat,
                    Expected::EndRepeatSymbol,
                    SyntacticPrimaryKind::RepeatedSequence,
                );
            }
            CommentlessSymbol::Symbol(Symbol::StartGroup) => {
                return Self::parse_bracketed(
                    parser,
                    span,
                    Symbol::EndGroup,
                    Expected::EndGroupSymbol,
                    SyntacticPrimaryKind::GroupedSequence,
                );
            }
            _ => return Ok(Self::empty(parser)),
        };
        parser.bump();
        Ok(Self { kind, span })
    }

    /// Reads a definitions list between the next token, found at `open`, and
    /// `close`.
    fn parse_bracketed(
        parser: &mut Parser<'_>,
        open: Span,
        close: Symbol,
        expected: Expected,
        kind: fn(DefinitionsList) -> SyntacticPrimaryKind,
    ) -> Result<Self, ParseError> {
        parser.bump();
        let definitions = DefinitionsList::parse(parser)?;
        let close = parser.expect(close, expected)?;
        Ok(Self {
            kind: kind(definitions),
            span: open.to(close),
        })
    }

    fn empty(parser: &Parser<'_>) -> Self {
        Self {
            kind: SyntacticPrimaryKind::EmptySequence,
            span: parser.empty_span(),
        }
    }
}
