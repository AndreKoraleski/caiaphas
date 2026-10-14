//! Bracketed textual comments of ISO/IEC 14977 EBNF.

use std::{error::Error, fmt};

use crate::lexer::{
    commentless_symbol::{
        special_sequence::{self, ParseSpecialSequenceError},
        terminal_string::{ParseTerminalStringError, TerminalString},
    },
    gap_separator,
    terminal_character::{Symbol, TerminalCharacter},
};

/// Returns how long the bracketed textual comment at the start of `input` is,
/// `(*` and `*)` included.
///
/// Comments nest, so `(* a (* b *) c *)` is one comment. Terminal strings and
/// special sequences inside a comment are read as such, so the `*)` in
/// `(* '*)' *)` does not end it, and a `'` in `(* don't *)` starts a terminal
/// string that has no closing quote.
pub(crate) fn length(
    input: &str,
) -> Result<usize, ParseBracketedTextualCommentError> {
    if !input.starts_with("(*") {
        return Err(ParseBracketedTextualCommentError::NoOpeningSymbol);
    }
    let mut depth = 1;
    let mut total = 2;
    loop {
        total += gap_separator::length(&input[total..]);
        let after = &input[total..];
        let mut chars = after.chars();
        let first = chars
            .next()
            .ok_or(ParseBracketedTextualCommentError::NoClosingSymbol)?;
        let (character, size) = TerminalCharacter::read(first, chars.next())
            .map_err(|_| {
            ParseBracketedTextualCommentError::InvalidCharacter(first)
        })?;
        total += match character {
            TerminalCharacter::Symbol(Symbol::StartComment) => {
                depth += 1;
                size
            }
            TerminalCharacter::Symbol(Symbol::EndComment) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(total + size);
                }
                size
            }
            TerminalCharacter::Symbol(
                Symbol::FirstQuote | Symbol::SecondQuote,
            ) => TerminalString::read(after)
                .map(|(_, length)| length)
                .map_err(ParseBracketedTextualCommentError::TerminalString)?,
            TerminalCharacter::Symbol(Symbol::SpecialSequence) => {
                special_sequence::length(after).map_err(
                    ParseBracketedTextualCommentError::SpecialSequence,
                )?
            }
            _ => size,
        };
    }
}

/// Error returned when text does not start with a valid bracketed textual
/// comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseBracketedTextualCommentError {
    /// The text does not start with `(*`.
    NoOpeningSymbol,
    /// The text ends before every `(*` has its `*)`.
    NoClosingSymbol,
    /// A character in the comment is neither a terminal character nor a gap.
    InvalidCharacter(char),
    /// A quote in the comment does not start a valid terminal string.
    TerminalString(ParseTerminalStringError),
    /// A `?` in the comment does not start a valid special sequence.
    SpecialSequence(ParseSpecialSequenceError),
}

impl fmt::Display for ParseBracketedTextualCommentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOpeningSymbol => {
                f.write_str("EBNF comment has no opening `(*`")
            }
            Self::NoClosingSymbol => {
                f.write_str("EBNF comment has no closing `*)`")
            }
            Self::InvalidCharacter(c) => {
                write!(f, "{c:?} is not a valid character in an EBNF comment")
            }
            Self::TerminalString(_) => {
                f.write_str("invalid EBNF terminal string in a comment")
            }
            Self::SpecialSequence(_) => {
                f.write_str("invalid EBNF special sequence in a comment")
            }
        }
    }
}

impl Error for ParseBracketedTextualCommentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TerminalString(error) => Some(error),
            Self::SpecialSequence(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_comment() {
        assert_eq!(length("(* a *)"), Ok(7));
        assert_eq!(length("(* a *) b = c;"), Ok(7));
    }

    #[test]
    fn reads_an_empty_comment() {
        assert_eq!(length("(**)"), Ok(4));
        assert_eq!(length("(* *)"), Ok(5));
    }

    #[test]
    fn reads_gaps_and_new_lines() {
        assert_eq!(length("(*\n\ta\r\n\tb\n*)"), Ok(12));
    }

    #[test]
    fn reads_nested_comments() {
        assert_eq!(length("(* a (* b *) c *)"), Ok(17));
        assert_eq!(length("(*(*(**)*)*)"), Ok(12));
    }

    #[test]
    fn reads_every_kind_of_terminal_character() {
        let input = "(* rule2 = a, [b] | {c} - (d) ; e. f! g/ 3 * h :) + @ *)";
        assert_eq!(length(input), Ok(input.len()));
    }

    #[test]
    fn reads_what_only_a_commentless_symbol_would_reject() {
        for input in ["(* 99999999999 *)", "(* ? any note ? *)", "(* ?? *)"] {
            assert_eq!(length(input), Ok(input.len()), "{input:?}");
        }
    }

    #[test]
    fn does_not_end_inside_a_terminal_string() {
        assert_eq!(length("(* '*)' *)"), Ok(10));
        assert_eq!(length("(* \"*)\" *)"), Ok(10));
    }

    #[test]
    fn does_not_nest_inside_a_terminal_string() {
        assert_eq!(length("(* '(*' *)"), Ok(10));
    }

    #[test]
    fn does_not_end_inside_a_special_sequence() {
        assert_eq!(length("(* ? *) ? *)"), Ok(12));
    }

    #[test]
    fn ends_at_the_longest_match() {
        assert_eq!(length("(* a*) b *)"), Ok(6));
    }

    #[test]
    fn rejects_text_without_an_opening_symbol() {
        for input in ["", "(", "( *", "*)", "a", " (* *)"] {
            assert_eq!(
                length(input),
                Err(ParseBracketedTextualCommentError::NoOpeningSymbol),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_text_without_a_closing_symbol() {
        for input in ["(*", "(*)", "(* a", "(* a * )", "(* (* a *)"] {
            assert_eq!(
                length(input),
                Err(ParseBracketedTextualCommentError::NoClosingSymbol),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_characters_that_are_neither_terminal_characters_nor_gaps() {
        for c in ['é', '\0', '\r', '\u{00A0}'] {
            assert_eq!(
                length(&format!("(* a{c}b *)")),
                Err(ParseBracketedTextualCommentError::InvalidCharacter(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_an_apostrophe_that_starts_an_unclosed_string() {
        assert_eq!(
            length("(* don't *)"),
            Err(ParseBracketedTextualCommentError::TerminalString(
                ParseTerminalStringError::NoClosingQuote
            ))
        );
        assert_eq!(
            length("(* don't\n*)"),
            Err(ParseBracketedTextualCommentError::TerminalString(
                ParseTerminalStringError::InvalidCharacter('\n')
            ))
        );
    }

    #[test]
    fn rejects_an_unclosed_special_sequence() {
        assert_eq!(
            length("(* is it? *)"),
            Err(ParseBracketedTextualCommentError::SpecialSequence(
                ParseSpecialSequenceError::NoClosingSymbol
            ))
        );
    }

    #[test]
    fn error_displays_message() {
        let cases = [
            (
                ParseBracketedTextualCommentError::NoOpeningSymbol,
                "EBNF comment has no opening `(*`",
            ),
            (
                ParseBracketedTextualCommentError::NoClosingSymbol,
                "EBNF comment has no closing `*)`",
            ),
            (
                ParseBracketedTextualCommentError::InvalidCharacter('é'),
                "'é' is not a valid character in an EBNF comment",
            ),
            (
                ParseBracketedTextualCommentError::TerminalString(
                    ParseTerminalStringError::NoClosingQuote,
                ),
                "invalid EBNF terminal string in a comment",
            ),
            (
                ParseBracketedTextualCommentError::SpecialSequence(
                    ParseSpecialSequenceError::NoClosingSymbol,
                ),
                "invalid EBNF special sequence in a comment",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn error_gives_the_wrapped_error_as_its_source() {
        let error = ParseBracketedTextualCommentError::TerminalString(
            ParseTerminalStringError::NoClosingQuote,
        );
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseTerminalStringError::NoClosingQuote.to_string())
        );
        let error = ParseBracketedTextualCommentError::SpecialSequence(
            ParseSpecialSequenceError::NoClosingSymbol,
        );
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseSpecialSequenceError::NoClosingSymbol.to_string())
        );
        assert!(
            ParseBracketedTextualCommentError::NoClosingSymbol
                .source()
                .is_none()
        );
    }
}
