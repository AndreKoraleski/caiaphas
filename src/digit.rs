//! Decimal digits of ISO/IEC 14977 EBNF.

#![cfg_attr(not(test), expect(dead_code, reason = "no lexer exists to read"))]

use std::{error::Error, fmt};

/// A decimal digit, `0` to `9`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Digit {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

impl TryFrom<char> for Digit {
    type Error = ParseDigitError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        Ok(match value {
            '0' => Self::Zero,
            '1' => Self::One,
            '2' => Self::Two,
            '3' => Self::Three,
            '4' => Self::Four,
            '5' => Self::Five,
            '6' => Self::Six,
            '7' => Self::Seven,
            '8' => Self::Eight,
            '9' => Self::Nine,
            _ => return Err(ParseDigitError(value)),
        })
    }
}

/// Error returned when a character is not a [`Digit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParseDigitError(char);

impl fmt::Display for ParseDigitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a valid EBNF decimal digit", self.0)
    }
}

impl Error for ParseDigitError {}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGITS: [Digit; 10] = [
        Digit::Zero,
        Digit::One,
        Digit::Two,
        Digit::Three,
        Digit::Four,
        Digit::Five,
        Digit::Six,
        Digit::Seven,
        Digit::Eight,
        Digit::Nine,
    ];

    #[test]
    fn parses_every_digit() {
        for (c, digit) in ('0'..='9').zip(DIGITS) {
            assert_eq!(Digit::try_from(c), Ok(digit), "{c:?}");
        }
    }

    #[test]
    fn rejects_every_other_ascii_character() {
        for c in (0..=0x7F_u8).map(char::from) {
            if !c.is_ascii_digit() {
                assert_eq!(
                    Digit::try_from(c),
                    Err(ParseDigitError(c)),
                    "{c:?}"
                );
            }
        }
    }

    #[test]
    fn rejects_non_ascii_digits() {
        // Characters counted as numeric, but not in EBNF.
        for c in [
            '٣',  // Arabic-Indic digit three
            '०',  // Devanagari digit zero
            '０', // fullwidth digit zero
            '𝟙',  // mathematical double-struck digit one
            '²',  // superscript two
            '½',  // vulgar fraction one half
            'Ⅳ',  // Roman numeral four
            '\u{10FFFF}',
        ] {
            assert_eq!(Digit::try_from(c), Err(ParseDigitError(c)), "{c:?}");
        }
    }

    #[test]
    fn ordering_follows_numeric_value() {
        assert!(DIGITS.is_sorted());
        assert!(Digit::Zero < Digit::Nine);
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            ParseDigitError('x').to_string(),
            "'x' is not a valid EBNF decimal digit"
        );
    }

    #[test]
    fn error_implements_error_trait() {
        let error: Box<dyn Error> = Box::new(ParseDigitError('x'));
        assert!(error.source().is_none());
    }
}
