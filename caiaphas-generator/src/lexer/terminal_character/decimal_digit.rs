//! Decimal digits of ISO/IEC 14977 EBNF.

/// A decimal digit from `0` to `9`.
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

impl DecimalDigit {
    /// Returns the decimal digit `c` is, if it is one.
    pub(crate) fn from_char(c: char) -> Option<Self> {
        Some(match c {
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
            _ => return None,
        })
    }
}

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
            assert_eq!(DecimalDigit::from_char(c), Some(digit), "{c:?}");
        }
    }

    #[test]
    fn rejects_every_other_ascii_character() {
        for c in (0..=0x7F_u8).map(char::from) {
            if !c.is_ascii_digit() {
                assert_eq!(DecimalDigit::from_char(c), None, "{c:?}");
            }
        }
    }

    #[test]
    fn rejects_non_ascii_digits() {
        // Characters counted as numeric, but not in EBNF.
        for c in [
            '٣',          // Arabic-Indic digit three
            '०',          // Devanagari digit zero
            '０',         // fullwidth digit zero
            '𝟙',          // mathematical double-struck digit one
            '²',          // superscript two
            '½',          // vulgar fraction one half
            'Ⅳ',          // Roman numeral four
            '\u{10FFFF}', // the last code point
        ] {
            assert_eq!(DecimalDigit::from_char(c), None, "{c:?}");
        }
    }

    #[test]
    fn ordering_follows_numeric_value() {
        assert!(DECIMAL_DIGITS.is_sorted());
        assert!(DecimalDigit::Zero < DecimalDigit::Nine);
    }
}
