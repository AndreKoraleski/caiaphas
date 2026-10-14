//! Letters of ISO/IEC 14977 EBNF.

#![cfg_attr(not(test), expect(dead_code, reason = "no lexer exists to read"))]

use std::{error::Error, fmt};

/// Whether a [`Letter`] is written in lowercase or uppercase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Case {
    Lowercase,
    Uppercase,
}

/// An ASCII letter, `a` to `z` or `A` to `Z`, with its [`Case`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Letter {
    A(Case),
    B(Case),
    C(Case),
    D(Case),
    E(Case),
    F(Case),
    G(Case),
    H(Case),
    I(Case),
    J(Case),
    K(Case),
    L(Case),
    M(Case),
    N(Case),
    O(Case),
    P(Case),
    Q(Case),
    R(Case),
    S(Case),
    T(Case),
    U(Case),
    V(Case),
    W(Case),
    X(Case),
    Y(Case),
    Z(Case),
}

impl TryFrom<char> for Letter {
    type Error = ParseLetterError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        let case = if value.is_ascii_lowercase() {
            Case::Lowercase
        } else {
            Case::Uppercase
        };

        Ok(match value.to_ascii_uppercase() {
            'A' => Self::A(case),
            'B' => Self::B(case),
            'C' => Self::C(case),
            'D' => Self::D(case),
            'E' => Self::E(case),
            'F' => Self::F(case),
            'G' => Self::G(case),
            'H' => Self::H(case),
            'I' => Self::I(case),
            'J' => Self::J(case),
            'K' => Self::K(case),
            'L' => Self::L(case),
            'M' => Self::M(case),
            'N' => Self::N(case),
            'O' => Self::O(case),
            'P' => Self::P(case),
            'Q' => Self::Q(case),
            'R' => Self::R(case),
            'S' => Self::S(case),
            'T' => Self::T(case),
            'U' => Self::U(case),
            'V' => Self::V(case),
            'W' => Self::W(case),
            'X' => Self::X(case),
            'Y' => Self::Y(case),
            'Z' => Self::Z(case),
            _ => return Err(ParseLetterError(value)),
        })
    }
}

/// Error returned when a character is not a [`Letter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParseLetterError(char);

impl fmt::Display for ParseLetterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a valid EBNF letter", self.0)
    }
}

impl Error for ParseLetterError {}

#[cfg(test)]
mod tests {
    use super::*;

    const LETTERS: [fn(Case) -> Letter; 26] = [
        Letter::A,
        Letter::B,
        Letter::C,
        Letter::D,
        Letter::E,
        Letter::F,
        Letter::G,
        Letter::H,
        Letter::I,
        Letter::J,
        Letter::K,
        Letter::L,
        Letter::M,
        Letter::N,
        Letter::O,
        Letter::P,
        Letter::Q,
        Letter::R,
        Letter::S,
        Letter::T,
        Letter::U,
        Letter::V,
        Letter::W,
        Letter::X,
        Letter::Y,
        Letter::Z,
    ];

    #[test]
    fn parses_every_lowercase_letter() {
        for (c, variant) in ('a'..='z').zip(LETTERS) {
            assert_eq!(
                Letter::try_from(c),
                Ok(variant(Case::Lowercase)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn parses_every_uppercase_letter() {
        for (c, variant) in ('A'..='Z').zip(LETTERS) {
            assert_eq!(
                Letter::try_from(c),
                Ok(variant(Case::Uppercase)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_every_other_ascii_character() {
        for c in (0..=0x7F_u8).map(char::from) {
            if !c.is_ascii_alphabetic() {
                assert_eq!(
                    Letter::try_from(c),
                    Err(ParseLetterError(c)),
                    "{c:?}"
                );
            }
        }
    }

    #[test]
    fn rejects_non_ascii_characters() {
        // Letters outside ASCII.
        for c in [
            'é',        // Latin small letter e with acute
            'ß',        // uppercases to "SS"
            'ı',        // dotless i, uppercases to 'I'
            'ſ',        // long s, uppercases to 'S'
            '\u{212A}', // Kelvin sign, lowercases to 'k'
            'Ａ',       // fullwidth A
            'А',        // Cyrillic A
            'Σ',
            '\u{00A0}', // no-break space
            '\u{10FFFF}',
        ] {
            assert_eq!(Letter::try_from(c), Err(ParseLetterError(c)), "{c:?}");
        }
    }

    #[test]
    fn case_distinguishes_letters() {
        assert_ne!(Letter::try_from('a'), Letter::try_from('A'));
    }

    #[test]
    fn ordering_follows_alphabet_then_case() {
        assert!(Letter::A(Case::Uppercase) < Letter::B(Case::Lowercase));
        assert!(Letter::A(Case::Lowercase) < Letter::A(Case::Uppercase));
        assert!(Case::Lowercase < Case::Uppercase);
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            ParseLetterError('1').to_string(),
            "'1' is not a valid EBNF letter"
        );
    }

    #[test]
    fn error_implements_error_trait() {
        let error: Box<dyn Error> = Box::new(ParseLetterError('1'));
        assert!(error.source().is_none());
    }
}
