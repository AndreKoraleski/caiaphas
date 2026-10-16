//! Errors tell where an input stops matching its grammar, and what could have
//! come there instead.

use std::fmt::{self, Write};

use crate::span::line_and_column;

/// Error returned when an input does not match its grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    offset: usize,
    line: usize,
    column: usize,
    expected: Vec<Expected>,
    found: Option<char>,
}

/// What a parser could have read where an [`Error`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Expected {
    /// This text.
    Text(&'static str),
    /// Any one character from the first to the last.
    Range(char, char),
    /// Any match of the rule with this name.
    Rule(&'static str),
    /// The end of the input.
    End,
}

impl Error {
    /// Returns the error at `offset` in `input`, where the parser could have
    /// read any of `expected`.
    pub(crate) fn new(
        input: &str,
        offset: usize,
        mut expected: Vec<Expected>,
    ) -> Self {
        expected.sort();
        expected.dedup();
        let (line, column) = line_and_column(input, offset);
        Self {
            offset,
            line,
            column,
            expected,
            found: input[offset..].chars().next(),
        }
    }

    /// Returns the byte offset in the input where the error is.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the line the error is on, counted from 1.
    pub fn line(&self) -> usize {
        self.line
    }

    /// Returns the column the error is at, counted in characters from 1.
    pub fn column(&self) -> usize {
        self.column
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => write!(f, "`{}`", Shown(text)),
            Self::Range(first, last) if first == last => {
                write!(f, "`{}`", Shown(&first.to_string()))
            }
            Self::Range(first, last) => write!(
                f,
                "a character from `{}` to `{}`",
                Shown(&first.to_string()),
                Shown(&last.to_string())
            ),
            Self::Rule(name) => {
                let vowels =
                    ['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U'];
                let article =
                    if name.starts_with(vowels) { "an" } else { "a" };
                write!(f, "{article} {name}")
            }
            Self::End => f.write_str("the end of the input"),
        }
    }
}

/// Shows text with its control characters escaped, so a tab reads `\t`.
struct Shown<'a>(&'a str);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if c.is_control() {
                write!(f, "{}", c.escape_debug())?;
            } else {
                f.write_char(c)?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.expected.is_empty() {
            f.write_str("expected ")?;
            for (index, item) in self.expected.iter().enumerate() {
                if index > 0 {
                    let last = index == self.expected.len() - 1;
                    f.write_str(if last { " or " } else { ", " })?;
                }
                write!(f, "{item}")?;
            }
        }
        match (self.expected.is_empty(), self.found) {
            (true, Some(c)) => {
                write!(f, "unexpected `{}`", Shown(&c.to_string()))?;
            }
            (true, None) => f.write_str("unexpected end of the input")?,
            (false, Some(c)) => {
                write!(f, ", found `{}`", Shown(&c.to_string()))?;
            }
            (false, None) => f.write_str(", found the end of the input")?,
        }
        write!(f, " at line {}, column {}", self.line, self.column)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_was_expected_and_found_where() {
        let error = Error::new(
            "{\"a\": 1]",
            7,
            vec![
                Expected::Text("}"),
                Expected::Text(","),
                Expected::Text("}"),
            ],
        );
        assert_eq!(
            error.to_string(),
            "expected `,` or `}`, found `]` at line 1, column 8"
        );
        assert_eq!((error.offset(), error.line(), error.column()), (7, 1, 8));
    }

    #[test]
    fn names_ranges_and_the_end_of_the_input() {
        let error = Error::new(
            "x\ny",
            3,
            vec![Expected::Range('0', '9'), Expected::Range('a', 'a')],
        );
        assert_eq!(
            error.to_string(),
            "expected a character from `0` to `9` or `a`, found the end of \
             the input at line 2, column 2"
        );
        assert_eq!((error.line(), error.column()), (2, 2));
        let error = Error::new("ab", 1, vec![Expected::End]);
        assert_eq!(
            error.to_string(),
            "expected the end of the input, found `b` at line 1, column 2"
        );
    }

    #[test]
    fn names_rules_with_their_article() {
        let error = Error::new(
            "]",
            0,
            vec![
                Expected::Rule("value"),
                Expected::Rule("object"),
                Expected::Rule("Element"),
            ],
        );
        assert_eq!(
            error.to_string(),
            "expected an Element, an object or a value, found `]` at line 1, \
             column 1"
        );
    }

    #[test]
    fn escapes_control_characters() {
        let error = Error::new("\t", 0, vec![Expected::Text("\n")]);
        assert_eq!(
            error.to_string(),
            "expected `\\n`, found `\\t` at line 1, column 1"
        );
    }

    #[test]
    fn says_what_it_found_when_nothing_in_particular_was_expected() {
        let error = Error::new("x", 0, Vec::new());
        assert_eq!(error.to_string(), "unexpected `x` at line 1, column 1");
        let error = Error::new("", 0, Vec::new());
        assert_eq!(
            error.to_string(),
            "unexpected end of the input at line 1, column 1"
        );
    }
}
