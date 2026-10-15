//! Letters of ISO/IEC 14977 EBNF.

/// Whether a [`Letter`] is written in lowercase or uppercase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Case {
    Lowercase,
    Uppercase,
}

/// An ASCII letter from `a` to `z` or `A` to `Z`, with its [`Case`].
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

impl Letter {
    /// Returns the letter `c` is, if it is one.
    pub(crate) fn from_char(c: char) -> Option<Self> {
        let case = if c.is_ascii_lowercase() {
            Case::Lowercase
        } else {
            Case::Uppercase
        };

        Some(match c.to_ascii_uppercase() {
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
            _ => return None,
        })
    }
}

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
                Letter::from_char(c),
                Some(variant(Case::Lowercase)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn parses_every_uppercase_letter() {
        for (c, variant) in ('A'..='Z').zip(LETTERS) {
            assert_eq!(
                Letter::from_char(c),
                Some(variant(Case::Uppercase)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_every_other_ascii_character() {
        for c in (0..=0x7F_u8).map(char::from) {
            if !c.is_ascii_alphabetic() {
                assert_eq!(Letter::from_char(c), None, "{c:?}");
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
            assert_eq!(Letter::from_char(c), None, "{c:?}");
        }
    }

    #[test]
    fn case_distinguishes_letters() {
        assert_ne!(Letter::from_char('a'), Letter::from_char('A'));
    }

    #[test]
    fn ordering_follows_alphabet_then_case() {
        assert!(Letter::A(Case::Uppercase) < Letter::B(Case::Lowercase));
        assert!(Letter::A(Case::Lowercase) < Letter::A(Case::Uppercase));
        assert!(Case::Lowercase < Case::Uppercase);
    }
}
