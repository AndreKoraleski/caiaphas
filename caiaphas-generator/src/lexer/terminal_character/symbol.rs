//! Symbols of ISO/IEC 14977 EBNF.

/// Punctuation of EBNF.
///
/// Symbols with more than one spelling do not record which one was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Symbol {
    /// `,`
    Concatenate,
    /// `=`
    Defining,
    /// `|`, `/` or `!`
    DefinitionSeparator,
    /// `*)`
    EndComment,
    /// `)`
    EndGroup,
    /// `]` or `/)`
    EndOption,
    /// `}` or `:)`
    EndRepeat,
    /// `-`
    Except,
    /// `'`
    FirstQuote,
    /// `*`
    Repetition,
    /// `"`
    SecondQuote,
    /// `?`
    SpecialSequence,
    /// `(*`
    StartComment,
    /// `(`
    StartGroup,
    /// `[` or `(/`
    StartOption,
    /// `{` or `(:`
    StartRepeat,
    /// `;` or `.`
    Terminator,
}

impl Symbol {
    /// Returns the symbol that `first` starts, and how many characters it
    /// takes, 1 or 2.
    ///
    /// `second` is the character after `first`, if any. A symbol takes it only
    /// when the two form a longer symbol, so `(*` is a start comment, but
    /// `( *` is a start group.
    pub(crate) fn read(
        first: char,
        second: Option<char>,
    ) -> Option<(Self, usize)> {
        Some(match first {
            '(' => match second {
                Some('*') => (Self::StartComment, 2),
                Some('/') => (Self::StartOption, 2),
                Some(':') => (Self::StartRepeat, 2),
                _ => (Self::StartGroup, 1),
            },
            '*' => match second {
                Some(')') => (Self::EndComment, 2),
                _ => (Self::Repetition, 1),
            },
            '/' => match second {
                Some(')') => (Self::EndOption, 2),
                _ => (Self::DefinitionSeparator, 1),
            },
            ':' => match second {
                Some(')') => (Self::EndRepeat, 2),
                _ => return None,
            },
            ',' => (Self::Concatenate, 1),
            '=' => (Self::Defining, 1),
            '|' | '!' => (Self::DefinitionSeparator, 1),
            ')' => (Self::EndGroup, 1),
            ']' => (Self::EndOption, 1),
            '}' => (Self::EndRepeat, 1),
            '-' => (Self::Except, 1),
            '\'' => (Self::FirstQuote, 1),
            '"' => (Self::SecondQuote, 1),
            '?' => (Self::SpecialSequence, 1),
            '[' => (Self::StartOption, 1),
            '{' => (Self::StartRepeat, 1),
            ';' | '.' => (Self::Terminator, 1),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every spelling, in the order ISO/IEC 14977 lists the symbols.
    const SPELLINGS: [(&str, Symbol); 24] = [
        (",", Symbol::Concatenate),
        ("=", Symbol::Defining),
        ("|", Symbol::DefinitionSeparator),
        ("/", Symbol::DefinitionSeparator),
        ("!", Symbol::DefinitionSeparator),
        ("*)", Symbol::EndComment),
        (")", Symbol::EndGroup),
        ("]", Symbol::EndOption),
        ("/)", Symbol::EndOption),
        ("}", Symbol::EndRepeat),
        (":)", Symbol::EndRepeat),
        ("-", Symbol::Except),
        ("'", Symbol::FirstQuote),
        ("*", Symbol::Repetition),
        ("\"", Symbol::SecondQuote),
        ("?", Symbol::SpecialSequence),
        ("(*", Symbol::StartComment),
        ("(", Symbol::StartGroup),
        ("[", Symbol::StartOption),
        ("(/", Symbol::StartOption),
        ("{", Symbol::StartRepeat),
        ("(:", Symbol::StartRepeat),
        (";", Symbol::Terminator),
        (".", Symbol::Terminator),
    ];

    fn read(text: &str) -> Option<(Symbol, usize)> {
        let mut chars = text.chars();
        let first = chars.next().expect("text is not empty");
        Symbol::read(first, chars.next())
    }

    #[test]
    fn reads_every_spelling() {
        for (text, symbol) in SPELLINGS {
            let length = text.chars().count();
            assert_eq!(read(text), Some((symbol, length)), "{text:?}");
        }
    }

    #[test]
    fn reads_one_character_at_the_end_of_input() {
        for (text, symbol) in SPELLINGS {
            if text.chars().count() == 1 {
                assert_eq!(read(text), Some((symbol, 1)), "{text:?}");
            }
        }
    }

    #[test]
    fn leaves_a_character_that_does_not_extend_the_symbol() {
        let cases = [
            ("(a", Symbol::StartGroup),
            ("*a", Symbol::Repetition),
            ("/a", Symbol::DefinitionSeparator),
            (")*", Symbol::EndGroup),
            ("((", Symbol::StartGroup),
            ("**", Symbol::Repetition),
        ];
        for (text, symbol) in cases {
            assert_eq!(read(text), Some((symbol, 1)), "{text:?}");
        }
    }

    #[test]
    fn prefers_the_longest_match() {
        // `(*)` is a start comment then an end group, not a start group then
        // an end comment.
        assert_eq!(read("(*)"), Some((Symbol::StartComment, 2)));
    }

    #[test]
    fn gaps_split_two_character_symbols() {
        for text in ["( *", "( /", "( :", "* )", "/ )"] {
            assert_eq!(
                read(text).map(|(_, length)| length),
                Some(1),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_a_colon_not_before_an_end_group() {
        for text in [":", ":a", "::", ": )"] {
            assert_eq!(read(text), None, "{text:?}");
        }
    }

    #[test]
    fn rejects_every_other_ascii_character() {
        for c in (0..=0x7F_u8).map(char::from) {
            if !SPELLINGS.iter().any(|(text, _)| text.starts_with(c)) {
                assert_eq!(Symbol::read(c, None), None, "{c:?}");
            }
        }
    }

    #[test]
    fn rejects_non_ascii_look_alikes() {
        // Characters that look like symbols, not in EBNF.
        for c in [
            '，',         // fullwidth comma
            '；',         // fullwidth semicolon
            '（',         // fullwidth left parenthesis
            '‘',          // left single quotation mark
            '“',          // left double quotation mark
            '−',          // minus sign
            '∗',          // asterisk operator
            '\u{10FFFF}', // the last code point
        ] {
            assert_eq!(Symbol::read(c, None), None, "{c:?}");
        }
    }

    #[test]
    fn ordering_follows_the_standard() {
        assert!(SPELLINGS.map(|(_, symbol)| symbol).is_sorted());
    }
}
