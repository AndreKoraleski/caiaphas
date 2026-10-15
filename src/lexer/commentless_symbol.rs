//! Commentless symbols of ISO/IEC 14977 EBNF.

mod integer;
mod meta_identifier;
pub(super) mod special_sequence;
pub(super) mod terminal_string;

use std::{error::Error, fmt};

pub(crate) use integer::Integer;
use integer::ParseIntegerError;
pub(crate) use meta_identifier::MetaIdentifier;
use special_sequence::ParseSpecialSequenceError;
pub(crate) use special_sequence::SpecialSequence;
use terminal_string::ParseTerminalStringError;
pub(crate) use terminal_string::TerminalString;

use crate::lexer::terminal_character::{Symbol, TerminalCharacter};

/// A commentless symbol is one of the symbols a grammar is made of once its
/// gaps and comments are removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommentlessSymbol {
    /// Any symbol but a quote, a start or end comment symbol or a special
    /// sequence symbol, since those start or end another commentless symbol or
    /// a comment.
    Symbol(Symbol),
    MetaIdentifier(MetaIdentifier),
    Integer(Integer),
    TerminalString(TerminalString),
    SpecialSequence(SpecialSequence),
}

impl CommentlessSymbol {
    /// Returns the commentless symbol at the start of `input`, and how long it
    /// is.
    ///
    /// `input` should not start with a gap. Gaps inside a meta identifier, an
    /// integer or a special sequence are part of its length.
    ///
    /// Every commentless symbol is ASCII, so the length is the same in bytes
    /// and characters.
    pub(crate) fn read(
        input: &str,
    ) -> Result<(Self, usize), ParseCommentlessSymbolError> {
        let mut chars = input.chars();
        let first = chars
            .next()
            .ok_or(ParseCommentlessSymbolError::EndOfInput)?;
        let (character, length) = TerminalCharacter::read(first, chars.next())
            .ok_or(ParseCommentlessSymbolError::InvalidCharacter(first))?;
        Ok(match character {
            TerminalCharacter::Letter(_) => {
                let (identifier, length) = MetaIdentifier::read(input).expect(
                    "text starting with a letter is a meta identifier",
                );
                (Self::MetaIdentifier(identifier), length)
            }
            TerminalCharacter::DecimalDigit(_) => {
                let (integer, length) = Integer::read(input)
                    .map_err(ParseCommentlessSymbolError::Integer)?;
                (Self::Integer(integer), length)
            }
            TerminalCharacter::Symbol(
                Symbol::FirstQuote | Symbol::SecondQuote,
            ) => {
                let (string, length) = TerminalString::read(input)
                    .map_err(ParseCommentlessSymbolError::TerminalString)?;
                (Self::TerminalString(string), length)
            }
            TerminalCharacter::Symbol(Symbol::SpecialSequence) => {
                let (sequence, length) = SpecialSequence::read(input)
                    .map_err(ParseCommentlessSymbolError::SpecialSequence)?;
                (Self::SpecialSequence(sequence), length)
            }
            TerminalCharacter::Symbol(Symbol::EndComment) => {
                return Err(
                    ParseCommentlessSymbolError::UnmatchedEndCommentSymbol,
                );
            }
            TerminalCharacter::Symbol(Symbol::StartComment)
            | TerminalCharacter::OtherCharacter(_) => {
                return Err(ParseCommentlessSymbolError::NotCommentless(
                    first,
                ));
            }
            TerminalCharacter::Symbol(symbol) => {
                (Self::Symbol(symbol), length)
            }
        })
    }
}

/// Error returned when text does not start with a valid [`CommentlessSymbol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseCommentlessSymbolError {
    /// The text is empty.
    EndOfInput,
    /// The text starts with a character that is not a terminal character.
    InvalidCharacter(char),
    /// The text starts with a terminal character that only belongs in a
    /// comment or a terminal string, such as an other character or `(*`.
    NotCommentless(char),
    /// The text starts with `*)` while no comment is open.
    UnmatchedEndCommentSymbol,
    /// The text starts with a decimal digit, but not a valid integer.
    Integer(ParseIntegerError),
    /// The text starts with a quote, but not a valid terminal string.
    TerminalString(ParseTerminalStringError),
    /// The text starts with `?`, but not a valid special sequence.
    SpecialSequence(ParseSpecialSequenceError),
}

impl fmt::Display for ParseCommentlessSymbolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndOfInput => f.write_str("expected an EBNF symbol"),
            Self::InvalidCharacter(c) => {
                write!(f, "{c:?} is not a valid EBNF terminal character")
            }
            Self::NotCommentless(c) => {
                write!(f, "{c:?} does not start an EBNF commentless symbol")
            }
            Self::UnmatchedEndCommentSymbol => {
                f.write_str("`*)` without a matching `(*`")
            }
            Self::Integer(_) => f.write_str("invalid EBNF integer"),
            Self::TerminalString(_) => {
                f.write_str("invalid EBNF terminal string")
            }
            Self::SpecialSequence(_) => {
                f.write_str("invalid EBNF special sequence")
            }
        }
    }
}

impl Error for ParseCommentlessSymbolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Integer(error) => Some(error),
            Self::TerminalString(error) => Some(error),
            Self::SpecialSequence(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(
        input: &str,
    ) -> Result<(CommentlessSymbol, usize), ParseCommentlessSymbolError> {
        CommentlessSymbol::read(input)
    }

    #[test]
    fn reads_symbols() {
        let cases = [
            (",", Symbol::Concatenate, 1),
            ("=", Symbol::Defining, 1),
            ("|", Symbol::DefinitionSeparator, 1),
            ("/", Symbol::DefinitionSeparator, 1),
            ("!", Symbol::DefinitionSeparator, 1),
            (")", Symbol::EndGroup, 1),
            ("]", Symbol::EndOption, 1),
            ("/)", Symbol::EndOption, 2),
            ("}", Symbol::EndRepeat, 1),
            (":)", Symbol::EndRepeat, 2),
            ("-", Symbol::Except, 1),
            ("*", Symbol::Repetition, 1),
            ("(", Symbol::StartGroup, 1),
            ("[", Symbol::StartOption, 1),
            ("(/", Symbol::StartOption, 2),
            ("{", Symbol::StartRepeat, 1),
            ("(:", Symbol::StartRepeat, 2),
            (";", Symbol::Terminator, 1),
            (".", Symbol::Terminator, 1),
        ];
        for (input, symbol, length) in cases {
            assert_eq!(
                read(input),
                Ok((CommentlessSymbol::Symbol(symbol), length)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn reads_meta_identifiers_with_their_gaps() {
        let (symbol, length) = read("syntax rule = x").unwrap();
        assert_eq!(length, 11);
        assert!(matches!(
            symbol,
            CommentlessSymbol::MetaIdentifier(identifier)
                if identifier.as_str() == "syntaxrule"
        ));
    }

    #[test]
    fn reads_integers_with_their_gaps() {
        let (symbol, length) = read("1 2 * x").unwrap();
        assert_eq!(length, 3);
        assert!(matches!(
            symbol,
            CommentlessSymbol::Integer(integer) if integer.get() == 12
        ));
    }

    #[test]
    fn reads_terminal_strings_in_either_quote() {
        for input in ["'a b' , c", "\"a b\" , c"] {
            let (symbol, length) = read(input).unwrap();
            assert_eq!(length, 5, "{input:?}");
            assert!(
                matches!(
                    symbol,
                    CommentlessSymbol::TerminalString(string)
                        if string.as_str() == "a b"
                ),
                "{input:?}"
            );
        }
    }

    #[test]
    fn reads_special_sequences_with_their_gaps() {
        let (symbol, length) = read("? U+0041 .. U+005A ? | x").unwrap();
        assert_eq!(length, 20);
        assert!(matches!(
            symbol,
            CommentlessSymbol::SpecialSequence(sequence)
                if *sequence.range() == ('A'..='Z')
        ));
    }

    #[test]
    fn reads_a_symbol_before_another() {
        assert_eq!(
            read("(*)"),
            Err(ParseCommentlessSymbolError::NotCommentless('('))
        );
        assert_eq!(
            read("( *)"),
            Ok((CommentlessSymbol::Symbol(Symbol::StartGroup), 1))
        );
    }

    #[test]
    fn rejects_a_start_comment_symbol() {
        assert_eq!(
            read("(* a *)"),
            Err(ParseCommentlessSymbolError::NotCommentless('('))
        );
    }

    #[test]
    fn rejects_an_unmatched_end_comment_symbol() {
        for input in ["*)", "*) a"] {
            assert_eq!(
                read(input),
                Err(ParseCommentlessSymbolError::UnmatchedEndCommentSymbol),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_other_characters() {
        for c in " :+_%@&#$<>\\^`~".chars() {
            assert_eq!(
                read(&c.to_string()),
                Err(ParseCommentlessSymbolError::NotCommentless(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_a_colon_not_before_an_end_group() {
        assert_eq!(
            read(": )"),
            Err(ParseCommentlessSymbolError::NotCommentless(':'))
        );
    }

    #[test]
    fn rejects_characters_that_are_not_terminal_characters() {
        for c in ['\t', '\n', '\r', '\0', 'é', '\u{00A0}'] {
            assert_eq!(
                read(&c.to_string()),
                Err(ParseCommentlessSymbolError::InvalidCharacter(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_empty_text() {
        assert_eq!(read(""), Err(ParseCommentlessSymbolError::EndOfInput));
    }

    #[test]
    fn passes_on_integer_errors() {
        assert_eq!(
            read("4294967296"),
            Err(ParseCommentlessSymbolError::Integer(
                ParseIntegerError::TooLarge
            ))
        );
    }

    #[test]
    fn passes_on_terminal_string_errors() {
        let cases = [
            ("''", ParseTerminalStringError::Empty),
            ("'abc", ParseTerminalStringError::NoClosingQuote),
            ("'a\tb'", ParseTerminalStringError::InvalidCharacter('\t')),
        ];
        for (input, error) in cases {
            assert_eq!(
                read(input),
                Err(ParseCommentlessSymbolError::TerminalString(error)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn passes_on_special_sequence_errors() {
        let cases = [
            ("?U+0041", ParseSpecialSequenceError::NoClosingSymbol),
            ("?a?", ParseSpecialSequenceError::Unsupported),
            (
                "?U+D800?",
                ParseSpecialSequenceError::NotAScalarValue(0xD800),
            ),
        ];
        for (input, error) in cases {
            assert_eq!(
                read(input),
                Err(ParseCommentlessSymbolError::SpecialSequence(error)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn error_displays_message() {
        let cases = [
            (
                ParseCommentlessSymbolError::EndOfInput,
                "expected an EBNF symbol",
            ),
            (
                ParseCommentlessSymbolError::InvalidCharacter('é'),
                "'é' is not a valid EBNF terminal character",
            ),
            (
                ParseCommentlessSymbolError::NotCommentless('@'),
                "'@' does not start an EBNF commentless symbol",
            ),
            (
                ParseCommentlessSymbolError::UnmatchedEndCommentSymbol,
                "`*)` without a matching `(*`",
            ),
            (
                ParseCommentlessSymbolError::Integer(
                    ParseIntegerError::TooLarge,
                ),
                "invalid EBNF integer",
            ),
            (
                ParseCommentlessSymbolError::TerminalString(
                    ParseTerminalStringError::Empty,
                ),
                "invalid EBNF terminal string",
            ),
            (
                ParseCommentlessSymbolError::SpecialSequence(
                    ParseSpecialSequenceError::Unsupported,
                ),
                "invalid EBNF special sequence",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn error_gives_the_wrapped_error_as_its_source() {
        let error =
            ParseCommentlessSymbolError::Integer(ParseIntegerError::TooLarge);
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseIntegerError::TooLarge.to_string())
        );
        let error = ParseCommentlessSymbolError::TerminalString(
            ParseTerminalStringError::Empty,
        );
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseTerminalStringError::Empty.to_string())
        );
        let error = ParseCommentlessSymbolError::SpecialSequence(
            ParseSpecialSequenceError::Unsupported,
        );
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseSpecialSequenceError::Unsupported.to_string())
        );
        assert!(ParseCommentlessSymbolError::EndOfInput.source().is_none());
    }
}
