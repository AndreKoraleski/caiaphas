//! Terminal characters of ISO/IEC 14977 EBNF.

mod decimal_digit;
mod letter;
mod other_character;
mod symbol;

use std::{error::Error, fmt};

use decimal_digit::DecimalDigit;
use letter::Letter;
use other_character::OtherCharacter;
pub(crate) use symbol::Symbol;

/// A terminal character is a letter, a decimal digit, a symbol or an other
/// character.
///
/// Some symbols, such as `(*`, are two characters long but are still a single
/// terminal character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalCharacter {
    Letter(Letter),
    DecimalDigit(DecimalDigit),
    Symbol(Symbol),
    OtherCharacter(OtherCharacter),
}

impl TerminalCharacter {
    /// Returns the terminal character that `first` starts, and how many
    /// characters it takes, 1 or 2.
    ///
    /// `second` is the character after `first`, if any. Only a symbol can take
    /// it.
    pub(crate) fn read(
        first: char,
        second: Option<char>,
    ) -> Result<(Self, usize), ParseTerminalCharacterError> {
        if let Ok(letter) = Letter::try_from(first) {
            return Ok((Self::Letter(letter), 1));
        }
        if let Ok(digit) = DecimalDigit::try_from(first) {
            return Ok((Self::DecimalDigit(digit), 1));
        }
        // `:` alone is an other character, but `:)` is a symbol, so the
        // longer symbol is tried first.
        if let Ok((symbol, length)) = Symbol::read(first, second) {
            return Ok((Self::Symbol(symbol), length));
        }
        OtherCharacter::try_from(first)
            .map(|other| (Self::OtherCharacter(other), 1))
            .map_err(|_| ParseTerminalCharacterError(first))
    }

    /// Returns whether this is a first terminal character.
    ///
    /// First terminal characters may appear in a terminal string between two
    /// `'`. They are every terminal character but the first quote symbol.
    pub(crate) fn is_first_terminal_character(self) -> bool {
        self != Self::Symbol(Symbol::FirstQuote)
    }

    /// Returns whether this is a second terminal character.
    ///
    /// Second terminal characters may appear in a terminal string between two
    /// `"`. They are every terminal character but the second quote symbol.
    pub(crate) fn is_second_terminal_character(self) -> bool {
        self != Self::Symbol(Symbol::SecondQuote)
    }
}

/// Error returned when a character does not start a [`TerminalCharacter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParseTerminalCharacterError(char);

impl fmt::Display for ParseTerminalCharacterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a valid EBNF terminal character", self.0)
    }
}

impl Error for ParseTerminalCharacterError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_letters() {
        assert_eq!(
            TerminalCharacter::read('a', None),
            Ok((TerminalCharacter::Letter(Letter::try_from('a').unwrap()), 1))
        );
        assert_eq!(
            TerminalCharacter::read('Z', Some('*')),
            Ok((TerminalCharacter::Letter(Letter::try_from('Z').unwrap()), 1))
        );
    }

    #[test]
    fn reads_digits() {
        assert_eq!(
            TerminalCharacter::read('7', Some(')')),
            Ok((
                TerminalCharacter::DecimalDigit(
                    DecimalDigit::try_from('7').unwrap()
                ),
                1
            ))
        );
    }

    #[test]
    fn reads_two_character_symbols() {
        assert_eq!(
            TerminalCharacter::read('(', Some('*')),
            Ok((TerminalCharacter::Symbol(Symbol::StartComment), 2))
        );
        assert_eq!(
            TerminalCharacter::read('(', Some(' ')),
            Ok((TerminalCharacter::Symbol(Symbol::StartGroup), 1))
        );
    }

    #[test]
    fn reads_a_colon_as_a_symbol_only_before_an_end_group() {
        assert_eq!(
            TerminalCharacter::read(':', Some(')')),
            Ok((TerminalCharacter::Symbol(Symbol::EndRepeat), 2))
        );
        for second in [None, Some(' '), Some(':'), Some('a')] {
            assert_eq!(
                TerminalCharacter::read(':', second),
                Ok((
                    TerminalCharacter::OtherCharacter(OtherCharacter::Colon),
                    1
                )),
                "{second:?}"
            );
        }
    }

    #[test]
    fn reads_other_characters() {
        assert_eq!(
            TerminalCharacter::read(' ', None),
            Ok((TerminalCharacter::OtherCharacter(OtherCharacter::Space), 1))
        );
        assert_eq!(
            TerminalCharacter::read('~', None),
            Ok((TerminalCharacter::OtherCharacter(OtherCharacter::Tilde), 1))
        );
    }

    #[test]
    fn accepts_exactly_printable_ascii_and_space() {
        for c in (0..=0x7F_u8).map(char::from) {
            let is_terminal = c == ' ' || c.is_ascii_graphic();
            assert_eq!(
                TerminalCharacter::read(c, None).is_ok(),
                is_terminal,
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_non_ascii_characters() {
        for c in ['é', '\u{00A0}', '＊', '（', '\u{10FFFF}'] {
            assert_eq!(
                TerminalCharacter::read(c, None),
                Err(ParseTerminalCharacterError(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_control_characters() {
        for c in ['\0', '\t', '\n', '\r', '\x0B', '\x0C', '\x7F'] {
            assert_eq!(
                TerminalCharacter::read(c, None),
                Err(ParseTerminalCharacterError(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn first_terminal_characters_exclude_only_the_first_quote() {
        for c in (0..=0x7F_u8).map(char::from) {
            if let Ok((character, _)) = TerminalCharacter::read(c, None) {
                assert_eq!(
                    character.is_first_terminal_character(),
                    c != '\'',
                    "{c:?}"
                );
            }
        }
    }

    #[test]
    fn second_terminal_characters_exclude_only_the_second_quote() {
        for c in (0..=0x7F_u8).map(char::from) {
            if let Ok((character, _)) = TerminalCharacter::read(c, None) {
                assert_eq!(
                    character.is_second_terminal_character(),
                    c != '"',
                    "{c:?}"
                );
            }
        }
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            ParseTerminalCharacterError('é').to_string(),
            "'é' is not a valid EBNF terminal character"
        );
    }

    #[test]
    fn error_implements_error_trait() {
        let error = ParseTerminalCharacterError('é');
        let error: &dyn Error = &error;
        assert!(error.source().is_none());
    }
}
