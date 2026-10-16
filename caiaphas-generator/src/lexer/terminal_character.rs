//! Terminal characters of ISO/IEC 14977 EBNF.

mod symbol;

pub(crate) use symbol::Symbol;

/// A terminal character is a letter, a decimal digit, a symbol or an other
/// character.
///
/// Some symbols, such as `(*`, are two characters long but are still a single
/// terminal character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalCharacter {
    /// An ASCII letter.
    Letter,
    /// A digit from `0` to `9`.
    DecimalDigit,
    Symbol(Symbol),
    /// One of the few other characters the standard lists, such as `_`.
    OtherCharacter,
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
    ) -> Option<(Self, usize)> {
        if first.is_ascii_alphabetic() {
            return Some((Self::Letter, 1));
        }
        if first.is_ascii_digit() {
            return Some((Self::DecimalDigit, 1));
        }
        // `:` alone is an other character, but `:)` is a symbol, so the longer
        // symbol is tried first.
        if let Some((symbol, length)) = Symbol::read(first, second) {
            return Some((Self::Symbol(symbol), length));
        }
        // In the order ISO/IEC 14977 lists them.
        matches!(
            first,
            ' ' | ':'
                | '+'
                | '_'
                | '%'
                | '@'
                | '&'
                | '#'
                | '$'
                | '<'
                | '>'
                | '\\'
                | '^'
                | '`'
                | '~'
        )
        .then_some((Self::OtherCharacter, 1))
    }

    /// Returns the terminal character at the start of `input`, and how many
    /// bytes it takes, 1 or 2. Every terminal character is ASCII, so that is
    /// as many characters.
    ///
    /// Returns `end` when `input` is empty, and `invalid` with the first
    /// character when it is not a terminal character, so each caller reports
    /// its own errors.
    pub(crate) fn read_first<E>(
        input: &str,
        end: E,
        invalid: impl FnOnce(char) -> E,
    ) -> Result<(Self, usize), E> {
        let mut chars = input.chars();
        let first = chars.next().ok_or(end)?;
        Self::read(first, chars.next()).ok_or_else(|| invalid(first))
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

/// Returns how to write `c` when it is not a terminal character, `quoted` or
/// not: as a special sequence, since one can hold any character, or as the
/// plain character it looks like, when that was meant.
pub(crate) fn how_to_write(c: char, quoted: bool) -> String {
    let special = format!("`? U+{:04X} ?`", u32::from(c));
    match (looks_like(c), quoted) {
        (Some(plain), true) => format!(
            "write `{plain}` if that was meant, or write the character itself \
             as {special}"
        ),
        (Some(plain), false) => format!("write `{plain}` if that was meant"),
        (None, true) => format!(
            "write it as {special} outside the quotes, since a special \
             sequence can hold any character"
        ),
        (None, false) if c.is_alphabetic() => format!(
            "write the rule name without `{c}`, since one only holds ASCII \
             letters, digits and spaces; to match `{c}`, write {special}"
        ),
        (None, false) => format!(
            "write it as {special} to match it, since a special sequence can \
             hold any character"
        ),
    }
}

/// Returns the terminal character that `c` looks like, when it is a space,
/// dash or quote that is easy to mistake for one.
fn looks_like(c: char) -> Option<char> {
    match c {
        '\u{00A0}'
        | '\u{2000}'..='\u{200A}'
        | '\u{202F}'
        | '\u{205F}'
        | '\u{3000}' => Some(' '),
        '\u{2010}'..='\u{2015}' | '\u{2212}' => Some('-'),
        '\u{2018}' | '\u{2019}' | '\u{2032}' => Some('\''),
        '\u{201C}' | '\u{201D}' | '\u{2033}' => Some('"'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_letters() {
        for (first, second) in [('a', None), ('Z', Some('*'))] {
            assert_eq!(
                TerminalCharacter::read(first, second),
                Some((TerminalCharacter::Letter, 1))
            );
        }
    }

    #[test]
    fn reads_digits() {
        for c in '0'..='9' {
            assert_eq!(
                TerminalCharacter::read(c, Some(')')),
                Some((TerminalCharacter::DecimalDigit, 1))
            );
        }
    }

    #[test]
    fn reads_two_character_symbols() {
        assert_eq!(
            TerminalCharacter::read('(', Some('*')),
            Some((TerminalCharacter::Symbol(Symbol::StartComment), 2))
        );
        assert_eq!(
            TerminalCharacter::read('(', Some(' ')),
            Some((TerminalCharacter::Symbol(Symbol::StartGroup), 1))
        );
    }

    #[test]
    fn reads_a_colon_as_a_symbol_only_before_an_end_group() {
        assert_eq!(
            TerminalCharacter::read(':', Some(')')),
            Some((TerminalCharacter::Symbol(Symbol::EndRepeat), 2))
        );
        for second in [None, Some(' '), Some(':'), Some('a')] {
            assert_eq!(
                TerminalCharacter::read(':', second),
                Some((TerminalCharacter::OtherCharacter, 1)),
                "{second:?}"
            );
        }
    }

    #[test]
    fn reads_the_first_terminal_character_of_text() {
        let read =
            |input| TerminalCharacter::read_first(input, "end", |_| "invalid");
        assert_eq!(
            read("(*a"),
            Ok((TerminalCharacter::Symbol(Symbol::StartComment), 2))
        );
        assert_eq!(read(""), Err("end"));
        assert_eq!(read("é"), Err("invalid"));
        assert_eq!(
            TerminalCharacter::read_first("é", None, Some),
            Err(Some('é'))
        );
    }

    #[test]
    fn reads_other_characters() {
        for c in " :+_%@&#$<>\\^`~".chars() {
            assert_eq!(
                TerminalCharacter::read(c, None),
                Some((TerminalCharacter::OtherCharacter, 1)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn accepts_exactly_printable_ascii_and_space() {
        for c in (0..=0x7F_u8).map(char::from) {
            let is_terminal = c == ' ' || c.is_ascii_graphic();
            assert_eq!(
                TerminalCharacter::read(c, None).is_some(),
                is_terminal,
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_non_ascii_characters() {
        for c in ['é', '\u{00A0}', '＊', '（', '\u{10FFFF}'] {
            assert_eq!(TerminalCharacter::read(c, None), None, "{c:?}");
        }
    }

    #[test]
    fn rejects_control_characters() {
        for c in ['\0', '\t', '\n', '\r', '\x0B', '\x0C', '\x7F'] {
            assert_eq!(TerminalCharacter::read(c, None), None, "{c:?}");
        }
    }

    #[test]
    fn first_terminal_characters_exclude_only_the_first_quote() {
        for c in (0..=0x7F_u8).map(char::from) {
            if let Some((character, _)) = TerminalCharacter::read(c, None) {
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
            if let Some((character, _)) = TerminalCharacter::read(c, None) {
                assert_eq!(
                    character.is_second_terminal_character(),
                    c != '"',
                    "{c:?}"
                );
            }
        }
    }
}
