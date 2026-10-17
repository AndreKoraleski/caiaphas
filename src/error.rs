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
    found: Option<String>,
    too_deep: bool,
}

/// What a parser could have read where an [`Error`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
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
    /// read any of `expected`. The text found there is the one character, or
    /// up to `rejected` when a rule's type did not read that text.
    pub(crate) fn new(
        input: &str,
        offset: usize,
        mut expected: Vec<Expected>,
        rejected: Option<usize>,
    ) -> Self {
        expected.sort();
        expected.dedup();
        // Ranges that touch read as one, so `[^"]` or `"` is any character.
        expected.dedup_by(|next, previous| match (next, previous) {
            (Expected::Range(first, next_last), Expected::Range(_, last))
                if *first <= after(*last) =>
            {
                *last = (*last).max(*next_last);
                true
            }
            _ => false,
        });
        let (line, column) = line_and_column(input, offset);
        let found = match rejected {
            Some(end) => Some(input[offset..end].to_owned()),
            None => input[offset..].chars().next().map(String::from),
        };
        Self {
            offset,
            line,
            column,
            expected,
            found,
            too_deep: false,
        }
    }

    /// Returns the error of an input whose matches nest more deeply at
    /// `offset` than a parser follows.
    pub(crate) fn too_deep(input: &str, offset: usize) -> Self {
        Self {
            too_deep: true,
            ..Self::new(input, offset, Vec::new(), None)
        }
    }

    /// Returns whether the input nests more deeply than a parser follows, as
    /// brackets inside brackets can, instead of not matching. A parser stops
    /// there so that it never runs out of stack.
    #[must_use]
    pub fn is_too_deep(&self) -> bool {
        self.too_deep
    }

    /// Returns the byte offset in the input where the error is.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the line the error is on, counted from 1.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// Returns the column the error is at, counted in characters from 1.
    #[must_use]
    pub fn column(&self) -> usize {
        self.column
    }

    /// Returns what could have come where the error is, in order.
    #[must_use]
    pub fn expected(&self) -> &[Expected] {
        &self.expected
    }

    /// Returns what was found where the error is: the character there, or all
    /// the text of a rule whose type did not read it. Returns `None` at the
    /// end of the input.
    #[must_use]
    pub fn found(&self) -> Option<&str> {
        self.found.as_deref()
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => write!(f, "`{}`", Shown(text)),
            Self::Range(first, last) if first == last => {
                write!(f, "`{}`", Shown(&first.to_string()))
            }
            Self::Range('\0', char::MAX) => f.write_str("any character"),
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

/// Returns the character after `c`, or `c` when it is the last.
fn after(c: char) -> char {
    match c {
        '\u{D7FF}' => '\u{E000}',
        _ => char::from_u32(u32::from(c) + 1).unwrap_or(c),
    }
}

/// Shows text with the characters that do not print escaped, so a tab reads
/// `\t` and the last code point `\u{10ffff}`.
struct Shown<'a>(&'a str);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if matches!(c, '"' | '\'' | '\\') {
                f.write_char(c)?;
            } else {
                write!(f, "{}", c.escape_debug())?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.too_deep {
            return write!(
                f,
                "the input nests too deeply to parse at line {}, column {}",
                self.line, self.column
            );
        }
        if !self.expected.is_empty() {
            f.write_str("expected ")?;
            let parts = described(&self.expected);
            for (index, part) in parts.iter().enumerate() {
                if index > 0 {
                    let last = index == parts.len() - 1;
                    f.write_str(if last { " or " } else { ", " })?;
                }
                f.write_str(part)?;
            }
        }
        match (self.expected.is_empty(), &self.found) {
            (true, Some(found)) => write!(f, "unexpected `{}`", Shown(found))?,
            (true, None) => f.write_str("unexpected end of the input")?,
            (false, Some(found)) => write!(f, ", found `{}`", Shown(found))?,
            (false, None) => f.write_str(", found the end of the input")?,
        }
        write!(f, " at line {}, column {}", self.line, self.column)
    }
}

impl std::error::Error for Error {}

/// Classes of characters with a name, by their ranges, the largest first.
const CLASSES: &[(&[(char, char)], &str)] = &[
    (&[('0', '9'), ('A', 'Z'), ('a', 'z')], "a letter or digit"),
    (&[('0', '9'), ('A', 'F'), ('a', 'f')], "a hexadecimal digit"),
    (&[('A', 'Z'), ('a', 'z')], "a letter"),
    (&[('0', '9')], "a digit"),
    (&[('a', 'z')], "a lowercase letter"),
    (&[('A', 'Z')], "an uppercase letter"),
];

/// Returns how to tell each of `expected`, with the ranges that make up a
/// class of characters with a name told by that name, where the first of them
/// is.
fn described(expected: &[Expected]) -> Vec<String> {
    let mut named: Vec<Option<&str>> = vec![None; expected.len()];
    let mut taken = vec![false; expected.len()];
    for &(ranges, name) in CLASSES {
        let found: Option<Vec<usize>> = ranges
            .iter()
            .map(|&(first, last)| {
                expected.iter().enumerate().position(|(index, item)| {
                    !taken[index] && *item == Expected::Range(first, last)
                })
            })
            .collect();
        if let Some(found) = found {
            for &index in &found {
                taken[index] = true;
            }
            named[found[0]] = Some(name);
        }
    }
    expected
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match (named[index], taken[index]) {
            (Some(name), _) => Some(name.to_owned()),
            (None, true) => None,
            (None, false) => Some(item.to_string()),
        })
        .collect()
}

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
            None,
        );
        assert_eq!(
            error.to_string(),
            "expected `,` or `}`, found `]` at line 1, column 8"
        );
        assert_eq!((error.offset(), error.line(), error.column()), (7, 1, 8));
        assert_eq!(
            error.expected(),
            [Expected::Text(","), Expected::Text("}")]
        );
        assert_eq!(error.found(), Some("]"));
    }

    #[test]
    fn joins_ranges_that_touch() {
        let error = Error::new(
            "\"",
            1,
            vec![
                Expected::Range('#', char::MAX),
                Expected::Range('"', '"'),
                Expected::Range('\0', '!'),
                Expected::Range('a', 'b'),
                Expected::Range('\u{E000}', '\u{E001}'),
                Expected::Range('\u{D000}', '\u{D7FF}'),
            ],
            None,
        );
        assert_eq!(error.expected(), [Expected::Range('\0', char::MAX)]);
        assert!(error.to_string().starts_with("expected any character,"));
        let error = Error::new(
            "",
            0,
            vec![
                Expected::Range('a', 'b'),
                Expected::Range('d', 'e'),
                Expected::Range('\u{E000}', '\u{E001}'),
                Expected::Range('\u{D000}', '\u{D7FF}'),
            ],
            None,
        );
        assert_eq!(
            error.expected(),
            [
                Expected::Range('a', 'b'),
                Expected::Range('d', 'e'),
                Expected::Range('\u{D000}', '\u{E001}'),
            ]
        );
    }

    #[test]
    fn names_the_classes_of_characters_that_ranges_make_up() {
        let error = Error::new(
            "",
            0,
            vec![
                Expected::Text("0x"),
                Expected::Range('a', 'f'),
                Expected::Range('0', '9'),
                Expected::Range('A', 'F'),
                Expected::Range('_', '_'),
            ],
            None,
        );
        assert_eq!(
            error.to_string(),
            "expected `0x`, a hexadecimal digit or `_`, found the end of the \
             input at line 1, column 1"
        );
        assert_eq!(error.expected().len(), 5);
        let error = Error::new(
            "",
            0,
            vec![
                Expected::Range('a', 'z'),
                Expected::Range('A', 'Z'),
                Expected::Range('0', '9'),
                Expected::Rule("name"),
            ],
            None,
        );
        assert!(
            error
                .to_string()
                .starts_with("expected a letter or digit or a name,"),
            "{error}"
        );
    }

    #[test]
    fn names_ranges_and_the_end_of_the_input() {
        let error = Error::new(
            "x\ny",
            3,
            vec![Expected::Range('0', '8'), Expected::Range('a', 'a')],
            None,
        );
        assert_eq!(
            error.to_string(),
            "expected a character from `0` to `8` or `a`, found the end of \
             the input at line 2, column 2"
        );
        assert_eq!((error.line(), error.column()), (2, 2));
        let error = Error::new("ab", 1, vec![Expected::End], None);
        assert_eq!(
            error.to_string(),
            "expected the end of the input, found `b` at line 1, column 2"
        );
    }

    #[test]
    fn shows_all_the_text_a_type_did_not_read() {
        let error = Error::new(
            "port 99999",
            5,
            vec![Expected::Rule("port")],
            Some(10),
        );
        assert_eq!(
            error.to_string(),
            "expected a port, found `99999` at line 1, column 6"
        );
        assert_eq!(error.found(), Some("99999"));
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
            None,
        );
        assert_eq!(
            error.to_string(),
            "expected an Element, an object or a value, found `]` at line 1, \
             column 1"
        );
    }

    #[test]
    fn escapes_control_characters() {
        let error = Error::new("\t", 0, vec![Expected::Text("\n")], None);
        assert_eq!(
            error.to_string(),
            "expected `\\n`, found `\\t` at line 1, column 1"
        );
    }

    #[test]
    fn escapes_what_does_not_print() {
        let error = Error::new(
            "\u{301}",
            0,
            vec![Expected::Range('#', char::MAX), Expected::Text("\"\\'")],
            None,
        );
        assert_eq!(
            error.to_string(),
            "expected `\"\\'` or a character from `#` to `\\u{10ffff}`, \
             found `\\u{301}` at line 1, column 1"
        );
    }

    #[test]
    fn says_what_it_found_when_nothing_in_particular_was_expected() {
        let error = Error::new("x", 0, Vec::new(), None);
        assert_eq!(error.to_string(), "unexpected `x` at line 1, column 1");
        let error = Error::new("", 0, Vec::new(), None);
        assert_eq!(
            error.to_string(),
            "unexpected end of the input at line 1, column 1"
        );
    }
}
