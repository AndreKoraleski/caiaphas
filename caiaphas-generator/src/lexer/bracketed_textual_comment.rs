//! Bracketed textual comments of ISO/IEC 14977 EBNF.

use std::{error::Error, fmt};

use crate::lexer::{
    commentless_symbol::{
        special_sequence::{self, ParseSpecialSequenceError},
        terminal_string::{self, ParseTerminalStringError},
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
#[cfg(test)]
pub(crate) fn length(
    input: &str,
) -> Result<usize, ParseBracketedTextualCommentError> {
    length_or_failure(input).map_err(|(error, _)| error)
}

/// Returns how long the bracketed textual comment at the start of `input` is,
/// as [`length`] does, or why it cannot be read and where in it that is.
pub(crate) fn length_or_failure(
    input: &str,
) -> Result<usize, (ParseBracketedTextualCommentError, usize)> {
    if !input.starts_with("(*") {
        return Err((
            ParseBracketedTextualCommentError::NoStartCommentSymbol,
            0,
        ));
    }
    let mut depth = 1;
    let mut total = 2;
    loop {
        total += gap_separator::length(&input[total..]);
        let after = &input[total..];
        let failed = |error| (error, total);
        let (character, size) = TerminalCharacter::read_first(
            after,
            ParseBracketedTextualCommentError::NoEndCommentSymbol,
            ParseBracketedTextualCommentError::InvalidCharacter,
        )
        .map_err(failed)?;
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
            ) => terminal_string::length(after).map_err(|error| {
                failed(ParseBracketedTextualCommentError::TerminalString(
                    error,
                ))
            })?,
            TerminalCharacter::Symbol(Symbol::SpecialSequence) => {
                special_sequence::length(after).map_err(|error| {
                    failed(ParseBracketedTextualCommentError::SpecialSequence(
                        error,
                    ))
                })?
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
    NoStartCommentSymbol,
    /// The text ends before every `(*` has its `*)`.
    NoEndCommentSymbol,
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
            Self::NoStartCommentSymbol => {
                f.write_str("comment has no opening `(*`")
            }
            Self::NoEndCommentSymbol => {
                f.write_str("comment has no closing `*)`")
            }
            Self::InvalidCharacter(c) => {
                write!(f, "{c:?} is not a valid character in a comment")
            }
            Self::TerminalString(ParseTerminalStringError::NoClosingQuote) => {
                f.write_str(
                    "a quote in this comment starts a terminal string that \
                     never closes",
                )
            }
            Self::TerminalString(_) => {
                f.write_str("invalid terminal string in a comment")
            }
            Self::SpecialSequence(_) => {
                f.write_str("invalid special sequence in a comment")
            }
        }
    }
}

impl ParseBracketedTextualCommentError {
    /// Returns whether the error is about a quote or `?` inside the comment,
    /// rather than the comment as a whole.
    pub(crate) fn is_about_a_part(self) -> bool {
        matches!(self, Self::TerminalString(_) | Self::SpecialSequence(_))
    }

    /// Returns how to fix the comment, when there is a usual way.
    pub(crate) fn help(self) -> Option<String> {
        match self {
            Self::NoEndCommentSymbol => Some("end it with `*)`".to_owned()),
            Self::TerminalString(ParseTerminalStringError::NoClosingQuote) => {
                Some(
                    "the standard reads quotes in a comment as terminal \
                     strings, so write `it is` rather than `it's`, or put the \
                     word in the other quotes, as in `\"it's\"`"
                        .to_owned(),
                )
            }
            Self::SpecialSequence(_) => Some(
                "the standard reads `?` in a comment as the start of a \
                 special sequence, so close it with another `?` or leave it \
                 out"
                .to_owned(),
            ),
            _ => None,
        }
    }
}

impl Error for ParseBracketedTextualCommentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TerminalString(ParseTerminalStringError::NoClosingQuote) => {
                None
            }
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
                Err(ParseBracketedTextualCommentError::NoStartCommentSymbol),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_text_without_a_closing_symbol() {
        for input in ["(*", "(*)", "(* a", "(* a * )", "(* (* a *)"] {
            assert_eq!(
                length(input),
                Err(ParseBracketedTextualCommentError::NoEndCommentSymbol),
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
                ParseTerminalStringError::NoClosingQuote
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
                ParseBracketedTextualCommentError::NoStartCommentSymbol,
                "comment has no opening `(*`",
            ),
            (
                ParseBracketedTextualCommentError::NoEndCommentSymbol,
                "comment has no closing `*)`",
            ),
            (
                ParseBracketedTextualCommentError::InvalidCharacter('é'),
                "'é' is not a valid character in a comment",
            ),
            (
                ParseBracketedTextualCommentError::TerminalString(
                    ParseTerminalStringError::NoClosingQuote,
                ),
                "a quote in this comment starts a terminal string that never \
                 closes",
            ),
            (
                ParseBracketedTextualCommentError::TerminalString(
                    ParseTerminalStringError::Empty,
                ),
                "invalid terminal string in a comment",
            ),
            (
                ParseBracketedTextualCommentError::SpecialSequence(
                    ParseSpecialSequenceError::NoClosingSymbol,
                ),
                "invalid special sequence in a comment",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn error_gives_the_wrapped_error_as_its_source() {
        let error = ParseBracketedTextualCommentError::TerminalString(
            ParseTerminalStringError::Empty,
        );
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseTerminalStringError::Empty.to_string())
        );
        let error = ParseBracketedTextualCommentError::SpecialSequence(
            ParseSpecialSequenceError::NoClosingSymbol,
        );
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseSpecialSequenceError::NoClosingSymbol.to_string())
        );
        assert!(
            ParseBracketedTextualCommentError::NoEndCommentSymbol
                .source()
                .is_none()
        );
    }
}
