//! Integers of ISO/IEC 14977 EBNF.

use std::{error::Error, fmt};

use crate::lexer::{gap_separator, terminal_character::TerminalCharacter};

/// An integer counts the repetitions of a syntactic factor.
///
/// It is one or more decimal digits. Gaps may come between them but are not
/// part of the number, so `1 2` is 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Integer(u32);

impl Integer {
    /// Returns the integer at the start of `input`, and how long it is, gaps
    /// between its digits included.
    pub(crate) fn read(
        input: &str,
    ) -> Result<(Self, usize), ParseIntegerError> {
        let (digits, length) = gap_separator::remove(input, is_decimal_digit);
        if digits.is_empty() {
            return Err(ParseIntegerError::NoDecimalDigit);
        }
        let value = digits.parse().map_err(|_| ParseIntegerError::TooLarge)?;
        Ok((Self(value), length))
    }

    pub(crate) fn get(self) -> u32 {
        self.0
    }
}

fn is_decimal_digit(c: char) -> bool {
    matches!(
        TerminalCharacter::read(c, None),
        Some((TerminalCharacter::DecimalDigit(_), _))
    )
}

/// Error returned when text does not start with a valid [`Integer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseIntegerError {
    /// The text does not start with a decimal digit.
    NoDecimalDigit,
    /// The digits are more than a `u32` can hold.
    TooLarge,
}

impl fmt::Display for ParseIntegerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoDecimalDigit => {
                "integer does not start with a decimal digit"
            }
            Self::TooLarge => "integer is too large to fit in a u32",
        })
    }
}

impl Error for ParseIntegerError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(input: &str) -> Result<(u32, usize), ParseIntegerError> {
        Integer::read(input).map(|(integer, length)| (integer.get(), length))
    }

    #[test]
    fn reads_digits() {
        assert_eq!(read("0"), Ok((0, 1)));
        assert_eq!(read("42"), Ok((42, 2)));
    }

    #[test]
    fn ignores_leading_zeros() {
        assert_eq!(read("007"), Ok((7, 3)));
    }

    #[test]
    fn removes_gaps_between_digits() {
        assert_eq!(read("1 2"), Ok((12, 3)));
        assert_eq!(read("1\n\t2 3"), Ok((123, 6)));
    }

    #[test]
    fn leaves_a_gap_after_the_last_digit() {
        assert_eq!(read("3 * 'a'"), Ok((3, 1)));
        assert_eq!(read("3*"), Ok((3, 1)));
    }

    #[test]
    fn stops_at_a_character_that_is_not_a_digit() {
        for input in ["1a", "1.5", "1,2", "1_000", "1٣"] {
            assert_eq!(read(input), Ok((1, 1)), "{input:?}");
        }
    }

    #[test]
    fn reads_the_largest_u32() {
        assert_eq!(read("4294967295"), Ok((u32::MAX, 10)));
    }

    #[test]
    fn rejects_integers_too_large_for_a_u32() {
        for input in ["4294967296", "4294 967 296", "99999999999999999999"] {
            assert_eq!(
                read(input),
                Err(ParseIntegerError::TooLarge),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_text_that_does_not_start_with_a_digit() {
        for input in ["", "a1", " 1", "-1", "+1", "٣"] {
            assert_eq!(
                read(input),
                Err(ParseIntegerError::NoDecimalDigit),
                "{input:?}"
            );
        }
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            ParseIntegerError::NoDecimalDigit.to_string(),
            "integer does not start with a decimal digit"
        );
        assert_eq!(
            ParseIntegerError::TooLarge.to_string(),
            "integer is too large to fit in a u32"
        );
    }
}
