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
pub(crate) enum Expected {
    Text(&'static str),
    Range(char, char),
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
            Self::Text(text) => write!(f, "`{text}`"),
            Self::Range(first, last) if first == last => {
                write!(f, "`{first}`")
            }
            Self::Range(first, last) => {
                write!(f, "a character from `{first}` to `{last}`")
            }
            Self::End => f.write_str("the end of the input"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut expected = String::new();
        for (index, item) in self.expected.iter().enumerate() {
            if index > 0 {
                let last = index == self.expected.len() - 1;
                expected.push_str(if last { " or " } else { ", " });
            }
            write!(expected, "{item}")?;
        }
        match self.found {
            Some(c) => write!(f, "expected {expected}, found `{c}`")?,
            None => {
                write!(f, "expected {expected}, found the end of the input")?
            }
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
}
