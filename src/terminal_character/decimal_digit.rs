//! Decimal digits of ISO/IEC 14977 EBNF.

use std::{error::Error, fmt};

/// A decimal digit, `0` to `9`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DecimalDigit {
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

impl TryFrom<char> for DecimalDigit {
    type Error = ParseDecimalDigitError;

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
            _ => return Err(ParseDecimalDigitError(value)),
        })
    }
}

/// Error returned when a character is not a [`DecimalDigit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParseDecimalDigitError(char);

impl fmt::Display for ParseDecimalDigitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a valid EBNF decimal digit", self.0)
    }
}

impl Error for ParseDecimalDigitError {}

#[cfg(test)]
mod tests {
    use super::*;

    const DECIMAL_DIGITS: [DecimalDigit; 10] = [
        DecimalDigit::Zero,
        DecimalDigit::One,
        DecimalDigit::Two,
        DecimalDigit::Three,
        DecimalDigit::Four,
        DecimalDigit::Five,
        DecimalDigit::Six,
        DecimalDigit::Seven,
        DecimalDigit::Eight,
        DecimalDigit::Nine,
    ];

    #[test]
    fn parses_every_digit() {
        for (c, digit) in ('0'..='9').zip(DECIMAL_DIGITS) {
            assert_eq!(DecimalDigit::try_from(c), Ok(digit), "{c:?}");
        }
    }

    #[test]
    fn rejects_every_other_ascii_character() {
        for c in (0..=0x7F_u8).map(char::from) {
            if !c.is_ascii_digit() {
                assert_eq!(
                    DecimalDigit::try_from(c),
                    Err(ParseDecimalDigitError(c)),
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
            assert_eq!(
                DecimalDigit::try_from(c),
                Err(ParseDecimalDigitError(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn ordering_follows_numeric_value() {
        assert!(DECIMAL_DIGITS.is_sorted());
        assert!(DecimalDigit::Zero < DecimalDigit::Nine);
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            ParseDecimalDigitError('x').to_string(),
            "'x' is not a valid EBNF decimal digit"
        );
    }

    #[test]
    fn error_implements_error_trait() {
        let error: Box<dyn Error> = Box::new(ParseDecimalDigitError('x'));
        assert!(error.source().is_none());
    }
}
