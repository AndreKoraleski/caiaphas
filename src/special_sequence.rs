//! Special sequences of ISO/IEC 14977 EBNF, read as Unicode code points.
//!
//! The standard leaves the meaning of a special sequence to each
//! implementation.
//!
//! Here it holds one code point, such as `? U+00E9 ?`, or an inclusive range
//! of them, such as `? U+0041..U+005A ?`.
#![cfg_attr(not(test), expect(dead_code, reason = "no lexer exists to read"))]

use std::{error::Error, fmt, ops::RangeInclusive};

use crate::{
    gap_separator,
    terminal_character::{Symbol, TerminalCharacter},
};

/// A special sequence, which stands for any one character in its range.
///
/// A single code point is a range from that code point to itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SpecialSequence(RangeInclusive<char>);

impl SpecialSequence {
    /// Returns the special sequence at the start of `input`, and how long it
    /// is, `?` included.
    ///
    /// Gaps between the `?` are removed before the code points are read, so
    /// `? U+0041 .. U+005A ?` is the same as `?U+0041..U+005A?`.
    ///
    /// Every special sequence character is ASCII, so the length is the same in
    /// bytes and characters.
    pub(crate) fn read(
        input: &str,
    ) -> Result<(Self, usize), ParseSpecialSequenceError> {
        if !input.starts_with('?') {
            return Err(ParseSpecialSequenceError::NoOpeningSymbol);
        }
        let mut content = String::new();
        let mut length = 1;
        loop {
            length += gap_separator::length(&input[length..]);
            let mut chars = input[length..].chars();
            let first = chars
                .next()
                .ok_or(ParseSpecialSequenceError::NoClosingSymbol)?;
            let (character, size) =
                TerminalCharacter::read(first, chars.next()).map_err(
                    |_| ParseSpecialSequenceError::InvalidCharacter(first),
                )?;
            // No two-character symbol ends in a `?`.
            if character == TerminalCharacter::Symbol(Symbol::SpecialSequence)
            {
                let range = parse_range(&content)?;
                return Ok((Self(range), length + 1));
            }
            content.push_str(&input[length..length + size]);
            length += size;
        }
    }

    /// Returns the characters it stands for.
    pub(crate) fn range(&self) -> &RangeInclusive<char> {
        &self.0
    }
}

/// Reads `U+XXXX` or `U+XXXX..U+YYYY`.
fn parse_range(
    content: &str,
) -> Result<RangeInclusive<char>, ParseSpecialSequenceError> {
    let (start, end) = match content.split_once("..") {
        Some((start, end)) => {
            (parse_code_point(start)?, parse_code_point(end)?)
        }
        None => {
            let c = parse_code_point(content)?;
            (c, c)
        }
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
        .map_err(|_| ParseSpecialSequenceError::Unsupported)?;
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
}

impl fmt::Display for ParseSpecialSequenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOpeningSymbol => {
                f.write_str("EBNF special sequence has no opening `?`")
            }
            Self::NoClosingSymbol => {
                f.write_str("EBNF special sequence has no closing `?`")
            }
            Self::InvalidCharacter(c) => write!(
                f,
                "{c:?} is not a valid character in an EBNF special sequence"
            ),
            Self::Unsupported => f.write_str(
                "EBNF special sequence must hold a code point such as \
                 U+00E9 or a range such as U+0041..U+005A",
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

impl Error for ParseSpecialSequenceError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(
        input: &str,
    ) -> Result<(RangeInclusive<char>, usize), ParseSpecialSequenceError> {
        SpecialSequence::read(input)
            .map(|(sequence, length)| (sequence.range().clone(), length))
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
                "EBNF special sequence has no opening `?`",
            ),
            (
                ParseSpecialSequenceError::NoClosingSymbol,
                "EBNF special sequence has no closing `?`",
            ),
            (
                ParseSpecialSequenceError::InvalidCharacter('é'),
                "'é' is not a valid character in an EBNF special sequence",
            ),
            (
                ParseSpecialSequenceError::Unsupported,
                "EBNF special sequence must hold a code point such as U+00E9 \
                 or a range such as U+0041..U+005A",
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

    #[test]
    fn error_implements_error_trait() {
        let error: &dyn Error = &ParseSpecialSequenceError::Unsupported;
        assert!(error.source().is_none());
    }
}
