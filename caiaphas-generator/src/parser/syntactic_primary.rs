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

/// A syntactic primary is what a syntactic factor repeats, such as a terminal
/// string or a group in brackets.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SyntacticPrimary {
    pub(crate) kind: SyntacticPrimaryKind,
    /// An empty sequence has an empty span where it would be.
    pub(crate) span: Span,
}

/// What a [`SyntacticPrimary`] is.
#[derive(Debug, PartialEq, Eq)]
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

/// Makes the kind of a sequence in brackets from what the brackets hold.
type Bracketed = fn(DefinitionsList) -> SyntacticPrimaryKind;

impl SyntacticPrimary {
    /// Reads a syntactic primary, or an empty sequence if the next token does
    /// not start one.
    pub(super) fn parse(parser: &mut Parser<'_>) -> Result<Self, ParseError> {
        let Some(token) = parser.peek() else {
            return Ok(Self::empty(parser));
        };
        let span = token.span;
        let bracketed: Option<(Symbol, Expected, Bracketed)> =
            match token.symbol {
                CommentlessSymbol::Symbol(Symbol::StartOption) => Some((
                    Symbol::EndOption,
                    Expected::EndOptionSymbol,
                    SyntacticPrimaryKind::OptionalSequence,
                )),
                CommentlessSymbol::Symbol(Symbol::StartRepeat) => Some((
                    Symbol::EndRepeat,
                    Expected::EndRepeatSymbol,
                    SyntacticPrimaryKind::RepeatedSequence,
                )),
                CommentlessSymbol::Symbol(Symbol::StartGroup) => Some((
                    Symbol::EndGroup,
                    Expected::EndGroupSymbol,
                    SyntacticPrimaryKind::GroupedSequence,
                )),
                _ => None,
            };
        if let Some((end, expected, kind)) = bracketed {
            return Self::parse_bracketed(parser, span, end, expected, kind);
        }
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
            _ => return Ok(Self::empty(parser)),
        };
        parser.bump();
        Ok(Self { kind, span })
    }

    /// Reads a definitions list between the next token, found at `start`, and
    /// the symbol `end`.
    fn parse_bracketed(
        parser: &mut Parser<'_>,
        start: Span,
        end: Symbol,
        expected: Expected,
        kind: Bracketed,
    ) -> Result<Self, ParseError> {
        parser.bump();
        let definitions = DefinitionsList::parse(parser)?;
        // `/)` ends an optional sequence, though a `/` before a `)` reads as
        // one.
        let expected = if end == Symbol::EndGroup
            && parser.peek().is_some_and(|token| {
                parser.lexer.source()[token.span.range()] == *"/)"
            }) {
            Expected::EndGroupSymbolNotOption
        } else {
            expected
        };
        let span = start.to(parser.expect(end, expected)?);
        Ok(Self {
            kind: kind(definitions),
            span,
        })
    }

    fn empty(parser: &Parser<'_>) -> Self {
        Self {
            kind: SyntacticPrimaryKind::EmptySequence,
            span: parser.empty_span(),
        }
    }
}
