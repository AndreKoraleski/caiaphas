//! Terminal strings of ISO/IEC 14977 EBNF.

use std::{error::Error, fmt};

use crate::lexer::terminal_character::TerminalCharacter;

/// A terminal string stands for its characters exactly as written.
///
/// It is written between two `'` or two `"`, and holds at least one terminal
/// character.
///
/// The quotes are not part of it, so `'a'` and `"a"` are the same terminal
/// string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TerminalString(String);

impl TerminalString {
    /// Returns the terminal string at the start of `input`, and how long it
    /// is, quotes included.
    pub(crate) fn read(
        input: &str,
    ) -> Result<(Self, usize), ParseTerminalStringError> {
        let is_content = match input.chars().next() {
            Some('\'') => TerminalCharacter::is_first_terminal_character,
            Some('"') => TerminalCharacter::is_second_terminal_character,
            _ => return Err(ParseTerminalStringError::NoOpeningQuote),
        };
        let mut length = 1;
        loop {
            let (character, size) = TerminalCharacter::read_first(
                &input[length..],
                ParseTerminalStringError::NoClosingQuote,
                ParseTerminalStringError::InvalidCharacter,
            )?;
            // No two-character symbol ends in a quote.
            if !is_content(character) {
                if length == 1 {
                    return Err(ParseTerminalStringError::Empty);
                }
                return Ok((Self(input[1..length].to_owned()), length + 1));
            }
            length += size;
        }
    }

    /// Returns the characters between the quotes.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Error returned when text does not start with a valid [`TerminalString`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseTerminalStringError {
    /// The text does not start with `'` or `"`.
    NoOpeningQuote,
    /// The text ends before the matching quote.
    NoClosingQuote,
    /// Nothing is between the quotes.
    Empty,
    /// A character between the quotes is not a terminal character.
    InvalidCharacter(char),
}

impl fmt::Display for ParseTerminalStringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOpeningQuote => {
                f.write_str("EBNF terminal string has no opening quote")
            }
            Self::NoClosingQuote => {
                f.write_str("EBNF terminal string has no closing quote")
            }
            Self::Empty => f.write_str("EBNF terminal string is empty"),
            Self::InvalidCharacter(c) => write!(
                f,
                "{c:?} is not a valid character in an EBNF terminal string"
            ),
        }
    }
}

impl Error for ParseTerminalStringError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(input: &str) -> Result<(String, usize), ParseTerminalStringError> {
        TerminalString::read(input)
            .map(|(string, length)| (string.as_str().to_owned(), length))
    }

    #[test]
    fn reads_between_either_quote() {
        assert_eq!(read("'abc'"), Ok(("abc".to_owned(), 5)));
        assert_eq!(read("\"abc\""), Ok(("abc".to_owned(), 5)));
    }

    #[test]
    fn ignores_which_quote_was_used() {
        assert_eq!(
            TerminalString::read("'a'").map(|(string, _)| string),
            TerminalString::read("\"a\"").map(|(string, _)| string)
        );
    }

    #[test]
    fn holds_the_other_quote() {
        assert_eq!(read("'\"'"), Ok(("\"".to_owned(), 3)));
        assert_eq!(read("\"'\""), Ok(("'".to_owned(), 3)));
        assert_eq!(read("'say \"hi\"'"), Ok(("say \"hi\"".to_owned(), 10)));
    }

    #[test]
    fn holds_every_terminal_character_but_its_quote() {
        let printable: String = (' '..='~').filter(|&c| c != '\'').collect();
        let quoted = format!("'{printable}'");
        assert_eq!(read(&quoted), Ok((printable, quoted.len())));
    }

    #[test]
    fn holds_symbols_without_reading_them() {
        for content in ["(*", "*)", "(/", "/)", "(:", ":)", ";", "?", "="] {
            let quoted = format!("'{content}'");
            assert_eq!(
                read(&quoted),
                Ok((content.to_owned(), quoted.len())),
                "{quoted:?}"
            );
        }
    }

    #[test]
    fn closes_after_a_character_that_starts_a_symbol() {
        for content in ["(", "*", "/", ":"] {
            let quoted = format!("'{content}'");
            assert_eq!(
                read(&quoted),
                Ok((content.to_owned(), 3)),
                "{quoted:?}"
            );
        }
    }

    #[test]
    fn stops_at_the_closing_quote() {
        assert_eq!(read("'a' , 'b'"), Ok(("a".to_owned(), 3)));
        assert_eq!(read("'a''b'"), Ok(("a".to_owned(), 3)));
    }

    #[test]
    fn rejects_text_without_an_opening_quote() {
        for input in ["", "a'", " 'a'", "?a?"] {
            assert_eq!(
                read(input),
                Err(ParseTerminalStringError::NoOpeningQuote),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_text_without_a_closing_quote() {
        for input in ["'", "'abc", "\"abc'", "'abc\""] {
            assert_eq!(
                read(input),
                Err(ParseTerminalStringError::NoClosingQuote),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_empty_strings() {
        assert_eq!(read("''"), Err(ParseTerminalStringError::Empty));
        assert_eq!(read("\"\""), Err(ParseTerminalStringError::Empty));
    }

    #[test]
    fn rejects_characters_that_are_not_terminal_characters() {
        for c in ['\t', '\n', '\r', '\0', 'é', '\u{00A0}'] {
            assert_eq!(
                read(&format!("'a{c}b'")),
                Err(ParseTerminalStringError::InvalidCharacter(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn error_displays_message() {
        let cases = [
            (
                ParseTerminalStringError::NoOpeningQuote,
                "EBNF terminal string has no opening quote",
            ),
            (
                ParseTerminalStringError::NoClosingQuote,
                "EBNF terminal string has no closing quote",
            ),
            (
                ParseTerminalStringError::Empty,
                "EBNF terminal string is empty",
            ),
            (
                ParseTerminalStringError::InvalidCharacter('é'),
                "'é' is not a valid character in an EBNF terminal string",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }
}
