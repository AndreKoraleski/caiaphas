//! Special sequences of ISO/IEC 14977 EBNF.
//!
//! The standard leaves the meaning of a special sequence to each
//! implementation.
//!
//! Here it holds one code point, such as `? U+00E9 ?`, an inclusive range of
//! them, such as `? U+0041..U+005A ?`, or a [`Pattern`] between two `/`, such
//! as `? /[a-z]+/ ?`. Anything else, such as a description in words, is not
//! read.

mod pattern;

use std::{error::Error, fmt, ops::RangeInclusive};

pub(crate) use pattern::Pattern;

use crate::lexer::{
    gap_separator,
    terminal_character::{Symbol, TerminalCharacter},
};

/// A special sequence stands for what its pattern matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpecialSequence(Pattern);

impl SpecialSequence {
    /// Returns the special sequence at the start of `input`, and how long it
    /// is, `?` included.
    ///
    /// Gaps between the `?` are removed before the code points are read, so
    /// `? U+0041 .. U+005A ?` is the same as `?U+0041..U+005A?`. In a pattern,
    /// only the gaps around the `/` are.
    pub(crate) fn read(
        input: &str,
    ) -> Result<(Self, usize), ParseSpecialSequenceError> {
        let length = length(input)?;
        let raw = input[1..length - 1].trim_matches(gap_separator::is_gap);
        if let Some(rest) = raw.strip_prefix('/') {
            let Some(text) = rest.strip_suffix('/') else {
                // A `?` inside a pattern ends the special sequence before the
                // pattern does.
                return Err(match meant_length(input, length) {
                    Some(meant) => {
                        ParseSpecialSequenceError::CutPattern(meant)
                    }
                    None => ParseSpecialSequenceError::UnclosedPattern,
                });
            };
            let pattern = pattern::parse(text)
                .map_err(ParseSpecialSequenceError::InvalidPattern)?;
            return Ok((Self(pattern), length));
        }
        // Between the `?`, every character that is not part of a gap is a
        // graphic ASCII character.
        let content: String =
            raw.chars().filter(char::is_ascii_graphic).collect();
        let range = parse_range(&content)?;
        Ok((Self(Pattern::Characters(vec![range])), length))
    }

    /// Returns what it matches.
    pub(crate) fn pattern(&self) -> &Pattern {
        &self.0
    }
}

/// Returns how long the special sequence at the start of `input` is, `?`
/// included.
pub(crate) fn length(input: &str) -> Result<usize, ParseSpecialSequenceError> {
    if !input.starts_with('?') {
        return Err(ParseSpecialSequenceError::NoOpeningSymbol);
    }
    let mut length = 1;
    loop {
        length += gap_separator::length(&input[length..]);
        let (character, size) = TerminalCharacter::read_first(
            &input[length..],
            ParseSpecialSequenceError::NoClosingSymbol,
            ParseSpecialSequenceError::InvalidCharacter,
        )?;
        // No two-character symbol holds a `?`, so going by terminal characters
        // cannot pass the closing one.
        if character == TerminalCharacter::Symbol(Symbol::SpecialSequence) {
            return Ok(length + 1);
        }
        length += size;
    }
}

/// Returns how long the special sequence at the start of `input` was meant to
/// be, when a `?` inside its pattern ended it at `length`: up to a `/` and a
/// `?` after it, with only spaces or tabs between, on the same line.
fn meant_length(input: &str, length: usize) -> Option<usize> {
    let line = input.find('\n').unwrap_or(input.len());
    let mut from = length;
    while let Some(slash) = input[from..line].find('/') {
        let after = from + slash + 1;
        let rest = &input[after..line];
        let gap = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        if rest[gap..].starts_with('?') {
            return Some(after + gap + 1);
        }
        from = after;
    }
    None
}

/// Reads `U+XXXX` or `U+XXXX..U+YYYY`.
fn parse_range(
    content: &str,
) -> Result<RangeInclusive<char>, ParseSpecialSequenceError> {
    let (start, end) = if let Some((start, end)) = content.split_once("..") {
        (parse_code_point(start)?, parse_code_point(end)?)
    } else {
        let c = parse_code_point(content)?;
        (c, c)
    };
    if start > end {
        return Err(ParseSpecialSequenceError::ReversedRange(start, end));
    }
    Ok(start..=end)
}

/// Reads `U+` followed by 4 to 6 uppercase hexadecimal digits.
fn parse_code_point(text: &str) -> Result<char, ParseSpecialSequenceError> {
    let digits = text
        .strip_prefix("U+")
        .filter(|digits| {
            (4..=6).contains(&digits.len())
                && digits.chars().all(|c| matches!(c, '0'..='9' | 'A'..='F'))
        })
        .ok_or(ParseSpecialSequenceError::Unsupported)?;
    let value = u32::from_str_radix(digits, 16)
        .expect("six hexadecimal digits fit in a u32");
    char::from_u32(value)
        .ok_or(ParseSpecialSequenceError::NotAScalarValue(value))
}

/// Error returned when text does not start with a valid [`SpecialSequence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseSpecialSequenceError {
    /// The text does not start with `?`.
    NoOpeningSymbol,
    /// The text ends before the closing `?`.
    NoClosingSymbol,
    /// A character between the `?` is neither a terminal character nor a gap.
    InvalidCharacter(char),
    /// What is between the `?` is not a code point or a range of them.
    Unsupported,
    /// A code point is a surrogate or above `U+10FFFF`.
    NotAScalarValue(u32),
    /// A range starts after it ends.
    ReversedRange(char, char),
    /// What is between the `/` is not a pattern, for this reason.
    InvalidPattern(&'static str),
    /// A pattern starts with `/` but does not end with one.
    UnclosedPattern,
    /// A `?` inside a pattern ended the special sequence, which was meant to
    /// be this long.
    CutPattern(usize),
}

impl fmt::Display for ParseSpecialSequenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOpeningSymbol => {
                f.write_str("special sequence has no opening `?`")
            }
            Self::NoClosingSymbol => {
                f.write_str("special sequence has no closing `?`")
            }
            Self::InvalidCharacter(c) => write!(
                f,
                "{c:?} is not a valid character in a special sequence"
            ),
            Self::Unsupported => f.write_str(
                "special sequence must hold a code point such as U+00E9, a \
                 range such as U+0041..U+005A or a pattern such as /[a-z]+/",
            ),
            Self::InvalidPattern(reason) => {
                write!(f, "invalid pattern in a special sequence: {reason}")
            }
            Self::UnclosedPattern => {
                f.write_str("pattern in a special sequence has no closing `/`")
            }
            Self::CutPattern(_) => f.write_str(
                "a `?` ends a special sequence, so this pattern stops before \
                 its closing `/`",
            ),
            Self::NotAScalarValue(value) => {
                write!(f, "U+{value:04X} is not a Unicode scalar value")
            }
            Self::ReversedRange(start, end) => write!(
                f,
                "range U+{:04X}..U+{:04X} ends before it starts",
                u32::from(*start),
                u32::from(*end)
            ),
        }
    }
}

impl ParseSpecialSequenceError {
    /// Returns how to fix the special sequence, when there is a usual way.
    pub(crate) fn help(self) -> Option<String> {
        match self {
            Self::CutPattern(_) => Some(
                "write `{0,1}` for an optional part of a pattern, as in \
                 `? /a{0,1}b/ ?`"
                    .to_owned(),
            ),
            Self::UnclosedPattern => Some(
                "end the pattern with `/`, as in `? /[a-z]+/ ?`".to_owned(),
            ),
            Self::NoClosingSymbol => Some(
                "end it with `?`; for an optional `x`, write `[x]` instead of \
                 `x?`"
                    .to_owned(),
            ),
            _ => None,
        }
    }
}

impl Error for ParseSpecialSequenceError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(
        input: &str,
    ) -> Result<(RangeInclusive<char>, usize), ParseSpecialSequenceError> {
        SpecialSequence::read(input).map(|(sequence, length)| {
            let Pattern::Characters(ranges) = sequence.pattern() else {
                panic!("a code point or range is one set of characters");
            };
            (ranges[0].clone(), length)
        })
    }

    #[test]
    fn reads_a_code_point() {
        assert_eq!(read("?U+00E9?"), Ok(('é'..='é', 8)));
    }

    #[test]
    fn reads_a_range() {
        assert_eq!(read("?U+0041..U+005A?"), Ok(('A'..='Z', 16)));
    }

    #[test]
    fn reads_a_range_of_one() {
        assert_eq!(read("?U+0041..U+0041?"), Ok(('A'..='A', 16)));
    }

    #[test]
    fn reads_four_to_six_digits() {
        assert_eq!(read("?U+0000?"), Ok(('\0'..='\0', 8)));
        assert_eq!(read("?U+1F600?"), Ok(('😀'..='😀', 9)));
        assert_eq!(read("?U+10FFFF?"), Ok(('\u{10FFFF}'..='\u{10FFFF}', 10)));
        assert_eq!(read("?U+000041?"), Ok(('A'..='A', 10)));
    }

    #[test]
    fn reads_a_range_across_the_surrogates() {
        assert_eq!(
            read("?U+D000..U+E000?"),
            Ok(('\u{D000}'..='\u{E000}', 16))
        );
    }

    #[test]
    fn removes_gaps() {
        assert_eq!(read("? U+0041 .. U+005A ?"), Ok(('A'..='Z', 20)));
        assert_eq!(read("?\n  U+00 E9\r\n?"), Ok(('é'..='é', 14)));
    }

    #[test]
    fn stops_at_the_closing_symbol() {
        assert_eq!(read("?U+0041? | 'a'"), Ok(('A'..='A', 8)));
        assert_eq!(read("?U+0041??U+0042?"), Ok(('A'..='A', 8)));
    }

    #[test]
    fn rejects_text_without_an_opening_symbol() {
        for input in ["", "U+0041?", " ?U+0041?", "'a'"] {
            assert_eq!(
                read(input),
                Err(ParseSpecialSequenceError::NoOpeningSymbol),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_text_without_a_closing_symbol() {
        for input in ["?", "?U+0041", "?U+0041 \n"] {
            assert_eq!(
                read(input),
                Err(ParseSpecialSequenceError::NoClosingSymbol),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_characters_that_are_neither_terminal_characters_nor_gaps() {
        for c in ['é', '\0', '\r', '\u{00A0}'] {
            assert_eq!(
                read(&format!("?U+00{c}41?")),
                Err(ParseSpecialSequenceError::InvalidCharacter(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_anything_but_code_points_and_ranges() {
        for content in [
            "",
            "LATIN SMALL LETTER E WITH ACUTE",
            "U+00e9",
            "u+00E9",
            "U00E9",
            "00E9",
            "U+E9",
            "U+00E",
            "U+0000041",
            "U+00G9",
            "U+0041..",
            "..U+005A",
            "U+0041-U+005A",
            "U+0041...U+005A",
            "U+0041..U+0042..U+0043",
            "'a'",
        ] {
            assert_eq!(
                read(&format!("?{content}?")),
                Err(ParseSpecialSequenceError::Unsupported),
                "{content:?}"
            );
        }
    }

    #[test]
    fn rejects_code_points_that_are_not_scalar_values() {
        let cases = [
            ("?U+D800?", 0xD800),
            ("?U+DFFF?", 0xDFFF),
            ("?U+110000?", 0x11_0000),
            ("?U+FFFFFF?", 0xFF_FFFF),
            ("?U+0041..U+D800?", 0xD800),
        ];
        for (input, value) in cases {
            assert_eq!(
                read(input),
                Err(ParseSpecialSequenceError::NotAScalarValue(value)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn measures_without_reading_the_content() {
        assert_eq!(length("? any note ? | x"), Ok(12));
        assert_eq!(length("??"), Ok(2));
        assert_eq!(
            length("?U+0041"),
            Err(ParseSpecialSequenceError::NoClosingSymbol)
        );
        assert_eq!(
            length("a?"),
            Err(ParseSpecialSequenceError::NoOpeningSymbol)
        );
        assert_eq!(
            length("? é ?"),
            Err(ParseSpecialSequenceError::InvalidCharacter('é'))
        );
    }

    #[test]
    fn reads_a_pattern_between_slashes() {
        let (sequence, length) = SpecialSequence::read("? /[ab]+/ ?").unwrap();
        assert_eq!(length, 11);
        assert!(matches!(sequence.pattern(), Pattern::Repeat { .. }));
        assert_eq!(
            SpecialSequence::read("?/[a/"),
            Err(ParseSpecialSequenceError::NoClosingSymbol)
        );
        assert_eq!(
            SpecialSequence::read("?/[a/?"),
            Err(ParseSpecialSequenceError::InvalidPattern(
                "a class needs a `]`"
            ))
        );
    }

    #[test]
    fn tells_a_pattern_that_a_question_mark_cuts_short() {
        assert_eq!(
            SpecialSequence::read("? /a?b/ ?;"),
            Err(ParseSpecialSequenceError::CutPattern(9))
        );
        assert_eq!(
            SpecialSequence::read("?/[+-]?[0-9]+/?"),
            Err(ParseSpecialSequenceError::CutPattern(15))
        );
        // Where no `/` and `?` follow on the line, the pattern is not closed.
        assert_eq!(
            SpecialSequence::read("? /[a-z]+ ?\nb = '/' ?"),
            Err(ParseSpecialSequenceError::UnclosedPattern)
        );
        assert_eq!(
            SpecialSequence::read("? / ?"),
            Err(ParseSpecialSequenceError::UnclosedPattern)
        );
    }

    #[test]
    fn rejects_reversed_ranges() {
        assert_eq!(
            read("?U+005A..U+0041?"),
            Err(ParseSpecialSequenceError::ReversedRange('Z', 'A'))
        );
    }

    #[test]
    fn error_displays_message() {
        let cases = [
            (
                ParseSpecialSequenceError::NoOpeningSymbol,
                "special sequence has no opening `?`",
            ),
            (
                ParseSpecialSequenceError::NoClosingSymbol,
                "special sequence has no closing `?`",
            ),
            (
                ParseSpecialSequenceError::InvalidCharacter('é'),
                "'é' is not a valid character in a special sequence",
            ),
            (
                ParseSpecialSequenceError::Unsupported,
                "special sequence must hold a code point such as U+00E9, a \
                 range such as U+0041..U+005A or a pattern such as /[a-z]+/",
            ),
            (
                ParseSpecialSequenceError::NotAScalarValue(0xD800),
                "U+D800 is not a Unicode scalar value",
            ),
            (
                ParseSpecialSequenceError::NotAScalarValue(0x11_0000),
                "U+110000 is not a Unicode scalar value",
            ),
            (
                ParseSpecialSequenceError::ReversedRange('Z', 'A'),
                "range U+005A..U+0041 ends before it starts",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }
}
