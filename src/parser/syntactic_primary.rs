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
    Optional(DefinitionsList),
    /// Matches the definitions between `{` and `}` any number of times.
    Repeated(DefinitionsList),
    /// Matches the definitions between `(` and `)` once.
    Grouped(DefinitionsList),
    MetaIdentifier(MetaIdentifier),
    TerminalString(TerminalString),
    SpecialSequence(SpecialSequence),
    /// Matches without reading anything.
    Empty,
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
                    Symbol::EndOption,
                    Expected::EndOptionSymbol,
                    SyntacticPrimaryKind::Optional,
                );
            }
            CommentlessSymbol::Symbol(Symbol::StartRepeat) => {
                return Self::parse_bracketed(
                    parser,
                    Symbol::EndRepeat,
                    Expected::EndRepeatSymbol,
                    SyntacticPrimaryKind::Repeated,
                );
            }
            CommentlessSymbol::Symbol(Symbol::StartGroup) => {
                return Self::parse_bracketed(
                    parser,
                    Symbol::EndGroup,
                    Expected::EndGroupSymbol,
                    SyntacticPrimaryKind::Grouped,
                );
            }
            _ => return Ok(Self::empty(parser)),
        };
        parser.bump();
        Ok(Self { kind, span })
    }

    /// Reads a definitions list between the next token and `close`.
    fn parse_bracketed(
        parser: &mut Parser<'_>,
        close: Symbol,
        expected: Expected,
        kind: fn(DefinitionsList) -> SyntacticPrimaryKind,
    ) -> Result<Self, ParseError> {
        let open = parser.bump().map(|token| token.span);
        let definitions = DefinitionsList::parse(parser)?;
        let close = parser.expect(close, expected)?;
        Ok(Self {
            kind: kind(definitions),
            span: open.map_or(close, |open| open.to(close)),
        })
    }

    fn empty(parser: &Parser<'_>) -> Self {
        Self {
            kind: SyntacticPrimaryKind::Empty,
            span: parser.empty_span(),
        }
    }
}
