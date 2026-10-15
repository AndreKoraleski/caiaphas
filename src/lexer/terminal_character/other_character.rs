//! Other characters of ISO/IEC 14977 EBNF.

use std::{error::Error, fmt};

/// One of the 15 ASCII characters, space included, that may appear in terminal
/// strings and comments but is not a symbol of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum OtherCharacter {
    Space,
    Colon,
    Plus,
    Underscore,
    Percent,
    At,
    Ampersand,
    Hash,
    Dollar,
    LessThan,
    GreaterThan,
    Backslash,
    Caret,
    Backtick,
    Tilde,
}

impl TryFrom<char> for OtherCharacter {
    type Error = ParseOtherCharacterError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        Ok(match value {
            ' ' => Self::Space,
            ':' => Self::Colon,
            '+' => Self::Plus,
            '_' => Self::Underscore,
            '%' => Self::Percent,
            '@' => Self::At,
            '&' => Self::Ampersand,
            '#' => Self::Hash,
            '$' => Self::Dollar,
            '<' => Self::LessThan,
            '>' => Self::GreaterThan,
            '\\' => Self::Backslash,
            '^' => Self::Caret,
            '`' => Self::Backtick,
            '~' => Self::Tilde,
            _ => return Err(ParseOtherCharacterError(value)),
        })
    }
}

/// Error returned when a character is not an [`OtherCharacter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParseOtherCharacterError(char);

impl fmt::Display for ParseOtherCharacterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a valid EBNF other character", self.0)
    }
}

impl Error for ParseOtherCharacterError {}

#[cfg(test)]
mod tests {
    use super::*;

    // In the order ISO/IEC 14977 lists them.
    const OTHER_CHARACTERS: [(char, OtherCharacter); 15] = [
        (' ', OtherCharacter::Space),
        (':', OtherCharacter::Colon),
        ('+', OtherCharacter::Plus),
        ('_', OtherCharacter::Underscore),
        ('%', OtherCharacter::Percent),
        ('@', OtherCharacter::At),
        ('&', OtherCharacter::Ampersand),
        ('#', OtherCharacter::Hash),
        ('$', OtherCharacter::Dollar),
        ('<', OtherCharacter::LessThan),
        ('>', OtherCharacter::GreaterThan),
        ('\\', OtherCharacter::Backslash),
        ('^', OtherCharacter::Caret),
        ('`', OtherCharacter::Backtick),
        ('~', OtherCharacter::Tilde),
    ];

    #[test]
    fn parses_every_other_character() {
        for (c, other) in OTHER_CHARACTERS {
            assert_eq!(OtherCharacter::try_from(c), Ok(other), "{c:?}");
        }
    }

    #[test]
    fn rejects_every_other_ascii_character() {
        for c in (0..=0x7F_u8).map(char::from) {
            if !OTHER_CHARACTERS.iter().any(|&(other, _)| other == c) {
                assert_eq!(
                    OtherCharacter::try_from(c),
                    Err(ParseOtherCharacterError(c)),
                    "{c:?}"
                );
            }
        }
    }

    #[test]
    fn rejects_non_ascii_look_alikes() {
        // Characters that look like other characters, not in EBNF.
        for c in [
            '\u{00A0}', // no-break space
            '\u{3000}', // ideographic space
            '：',       // fullwidth colon
            '＋',       // fullwidth plus sign
            '﹫',       // small commercial at
            '＼',       // fullwidth reverse solidus
            '˜',        // small tilde
            '∼',        // tilde operator
            '\u{10FFFF}',
        ] {
            assert_eq!(
                OtherCharacter::try_from(c),
                Err(ParseOtherCharacterError(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn ordering_follows_the_standard() {
        assert!(OTHER_CHARACTERS.map(|(_, other)| other).is_sorted());
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            ParseOtherCharacterError('a').to_string(),
            "'a' is not a valid EBNF other character"
        );
    }

    #[test]
    fn error_implements_error_trait() {
        let error: Box<dyn Error> = Box::new(ParseOtherCharacterError('a'));
        assert!(error.source().is_none());
    }
}
