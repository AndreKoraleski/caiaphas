//! Commentless symbols of ISO/IEC 14977 EBNF.

mod integer;
mod meta_identifier;
pub(super) mod special_sequence;
pub(super) mod terminal_string;

use std::{error::Error, fmt};

pub(crate) use integer::Integer;
use integer::ParseIntegerError;
pub(crate) use meta_identifier::MetaIdentifier;
use special_sequence::ParseSpecialSequenceError;
pub(crate) use special_sequence::{Pattern, SpecialSequence};
use terminal_string::ParseTerminalStringError;
pub(crate) use terminal_string::TerminalString;

use crate::lexer::{
    shown,
    terminal_character::{Symbol, TerminalCharacter},
};

/// A commentless symbol is one of the symbols a grammar is made of once its
/// gaps and comments are removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommentlessSymbol {
    /// Any symbol but a quote, a start or end comment symbol or a special
    /// sequence symbol, since those start or end another commentless symbol or
    /// a comment.
    Symbol(Symbol),
    MetaIdentifier(MetaIdentifier),
    Integer(Integer),
    TerminalString(TerminalString),
    SpecialSequence(SpecialSequence),
}

impl CommentlessSymbol {
    /// Returns the commentless symbol at the start of `input`, and how long it
    /// is.
    ///
    /// `input` should not start with a gap. Gaps inside a meta identifier, an
    /// integer or a special sequence are part of its length.
    ///
    /// Every commentless symbol is ASCII, so the length is the same in bytes
    /// and characters.
    pub(crate) fn read(
        input: &str,
    ) -> Result<(Self, usize), ParseCommentlessSymbolError> {
        let (character, length) = TerminalCharacter::read_first(
            input,
            ParseCommentlessSymbolError::EndOfInput,
            ParseCommentlessSymbolError::InvalidCharacter,
        )?;
        Ok(match character {
            TerminalCharacter::Letter => {
                let (identifier, length) = MetaIdentifier::read(input)
                    .expect("text starting with a letter is a rule name");
                (Self::MetaIdentifier(identifier), length)
            }
            TerminalCharacter::DecimalDigit => {
                let (integer, length) = Integer::read(input)
                    .map_err(ParseCommentlessSymbolError::Integer)?;
                (Self::Integer(integer), length)
            }
            TerminalCharacter::Symbol(
                Symbol::FirstQuote | Symbol::SecondQuote,
            ) => {
                let (string, length) = TerminalString::read(input)
                    .map_err(ParseCommentlessSymbolError::TerminalString)?;
                (Self::TerminalString(string), length)
            }
            TerminalCharacter::Symbol(Symbol::SpecialSequence) => {
                let (sequence, length) = SpecialSequence::read(input)
                    .map_err(ParseCommentlessSymbolError::SpecialSequence)?;
                (Self::SpecialSequence(sequence), length)
            }
            TerminalCharacter::Symbol(Symbol::EndComment) => {
                return Err(
                    ParseCommentlessSymbolError::UnmatchedEndCommentSymbol,
                );
            }
            TerminalCharacter::Symbol(Symbol::StartComment)
            | TerminalCharacter::OtherCharacter => {
                let first = input
                    .chars()
                    .next()
                    .expect("text with a terminal character is not empty");
                return Err(ParseCommentlessSymbolError::NotCommentless(
                    first,
                ));
            }
            TerminalCharacter::Symbol(symbol) => {
                (Self::Symbol(symbol), length)
            }
        })
    }
}

/// Error returned when text does not start with a valid [`CommentlessSymbol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseCommentlessSymbolError {
    /// The text is empty.
    EndOfInput,
    /// The text starts with a character that is not a terminal character.
    InvalidCharacter(char),
    /// The text starts with a terminal character that only belongs in a
    /// comment or a terminal string, such as an other character or `(*`.
    NotCommentless(char),
    /// The text starts with `*)` while no comment is open.
    UnmatchedEndCommentSymbol,
    /// The text starts with a decimal digit, but not a valid integer.
    Integer(ParseIntegerError),
    /// The text starts with a quote, but not a valid terminal string.
    TerminalString(ParseTerminalStringError),
    /// The text starts with `?`, but not a valid special sequence.
    SpecialSequence(ParseSpecialSequenceError),
}

impl fmt::Display for ParseCommentlessSymbolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndOfInput => f.write_str("expected a symbol"),
            Self::InvalidCharacter(c) | Self::NotCommentless(c) => {
                write!(f, "unexpected character {}", shown(*c))
            }
            Self::UnmatchedEndCommentSymbol => {
                f.write_str("`*)` without a matching `(*`")
            }
            // Each of these says itself what is wrong.
            Self::Integer(error) => error.fmt(f),
            Self::TerminalString(error) => error.fmt(f),
            Self::SpecialSequence(error) => error.fmt(f),
        }
    }
}

impl ParseCommentlessSymbolError {
    /// Returns how to fix the symbol, when there is a usual way.
    pub(crate) fn help(self) -> Option<String> {
        match self {
            Self::InvalidCharacter(c) => {
                Some(crate::lexer::terminal_character::how_to_write(c, false))
            }
            Self::NotCommentless(c) => Some(how_to_write_outside(c)),
            Self::TerminalString(error) => error.help(),
            Self::SpecialSequence(error) => error.help(),
            _ => None,
        }
    }
}

/// Returns how to write what the character `c` stands for, where it cannot
/// stand outside a terminal string, as with the notation of other grammars.
fn how_to_write_outside(c: char) -> String {
    match c {
        ':' => "write `=` to define a rule, as in `a = 'x';`".to_owned(),
        '_' => "write a space instead, since a rule name holds letters, \
                digits and spaces"
            .to_owned(),
        '+' => "for one or more of `x`, write `x, {x}`".to_owned(),
        '<' | '>' => "write a rule name without `<` and `>`".to_owned(),
        '#' => "write a comment as `(* a comment *)`, or `'#'` in quotes to \
                match it"
            .to_owned(),
        _ => format!("write it in quotes to match it, as in `'{c}'`"),
    }
}

impl Error for ParseCommentlessSymbolError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(
        input: &str,
    ) -> Result<(CommentlessSymbol, usize), ParseCommentlessSymbolError> {
        CommentlessSymbol::read(input)
    }

    #[test]
    fn reads_symbols() {
        let cases = [
            (",", Symbol::Concatenate, 1),
            ("=", Symbol::Defining, 1),
            ("|", Symbol::DefinitionSeparator, 1),
            ("/", Symbol::DefinitionSeparator, 1),
            ("!", Symbol::DefinitionSeparator, 1),
            (")", Symbol::EndGroup, 1),
            ("]", Symbol::EndOption, 1),
            ("/)", Symbol::EndOption, 2),
            ("}", Symbol::EndRepeat, 1),
            (":)", Symbol::EndRepeat, 2),
            ("-", Symbol::Except, 1),
            ("*", Symbol::Repetition, 1),
            ("(", Symbol::StartGroup, 1),
            ("[", Symbol::StartOption, 1),
            ("(/", Symbol::StartOption, 2),
            ("{", Symbol::StartRepeat, 1),
            ("(:", Symbol::StartRepeat, 2),
            (";", Symbol::Terminator, 1),
            (".", Symbol::Terminator, 1),
        ];
        for (input, symbol, length) in cases {
            assert_eq!(
                read(input),
                Ok((CommentlessSymbol::Symbol(symbol), length)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn reads_meta_identifiers_with_their_gaps() {
        let (symbol, length) = read("syntax rule = x").unwrap();
        assert_eq!(length, 11);
        assert!(matches!(
            symbol,
            CommentlessSymbol::MetaIdentifier(identifier)
                if identifier.as_str() == "syntaxrule"
        ));
    }

    #[test]
    fn reads_integers_with_their_gaps() {
        let (symbol, length) = read("1 2 * x").unwrap();
        assert_eq!(length, 3);
        assert!(matches!(
            symbol,
            CommentlessSymbol::Integer(integer) if integer.get() == 12
        ));
    }

    #[test]
    fn reads_terminal_strings_in_either_quote() {
        for input in ["'a b' , c", "\"a b\" , c"] {
            let (symbol, length) = read(input).unwrap();
            assert_eq!(length, 5, "{input:?}");
            assert!(
                matches!(
                    symbol,
                    CommentlessSymbol::TerminalString(string)
                        if string.as_str() == "a b"
                ),
                "{input:?}"
            );
        }
    }

    #[test]
    fn reads_special_sequences_with_their_gaps() {
        let (symbol, length) = read("? U+0041 .. U+005A ? | x").unwrap();
        assert_eq!(length, 20);
        let CommentlessSymbol::SpecialSequence(sequence) = symbol else {
            panic!("a special sequence");
        };
        let Pattern::Characters(ranges) = sequence.pattern() else {
            panic!("a range");
        };
        assert_eq!(ranges.first(), Some(&('A'..='Z')));
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn splits_a_spaced_start_comment_symbol() {
        assert_eq!(
            read("( *)"),
            Ok((CommentlessSymbol::Symbol(Symbol::StartGroup), 1))
        );
    }

    #[test]
    fn rejects_a_start_comment_symbol() {
        assert_eq!(
            read("(* a *)"),
            Err(ParseCommentlessSymbolError::NotCommentless('('))
        );
    }

    #[test]
    fn rejects_an_unmatched_end_comment_symbol() {
        for input in ["*)", "*) a"] {
            assert_eq!(
                read(input),
                Err(ParseCommentlessSymbolError::UnmatchedEndCommentSymbol),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_other_characters() {
        for c in " :+_%@&#$<>\\^`~".chars() {
            assert_eq!(
                read(&c.to_string()),
                Err(ParseCommentlessSymbolError::NotCommentless(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_a_colon_not_before_an_end_group() {
        assert_eq!(
            read(": )"),
            Err(ParseCommentlessSymbolError::NotCommentless(':'))
        );
    }

    #[test]
    fn rejects_characters_that_are_not_terminal_characters() {
        for c in ['\t', '\n', '\r', '\0', 'é', '\u{00A0}'] {
            assert_eq!(
                read(&c.to_string()),
                Err(ParseCommentlessSymbolError::InvalidCharacter(c)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rejects_empty_text() {
        assert_eq!(read(""), Err(ParseCommentlessSymbolError::EndOfInput));
    }

    #[test]
    fn passes_on_integer_errors() {
        assert_eq!(
            read("4294967296"),
            Err(ParseCommentlessSymbolError::Integer(
                ParseIntegerError::TooLarge
            ))
        );
    }

    #[test]
    fn passes_on_terminal_string_errors() {
        let cases = [
            ("''", ParseTerminalStringError::Empty),
            ("'abc", ParseTerminalStringError::NoClosingQuote),
            ("'a\tb'", ParseTerminalStringError::InvalidCharacter('\t')),
        ];
        for (input, error) in cases {
            assert_eq!(
                read(input),
                Err(ParseCommentlessSymbolError::TerminalString(error)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn passes_on_special_sequence_errors() {
        let cases = [
            ("?U+0041", ParseSpecialSequenceError::NoClosingSymbol),
            ("?a?", ParseSpecialSequenceError::Unsupported),
            (
                "?U+D800?",
                ParseSpecialSequenceError::NotAScalarValue(0xD800),
            ),
        ];
        for (input, error) in cases {
            assert_eq!(
                read(input),
                Err(ParseCommentlessSymbolError::SpecialSequence(error)),
                "{input:?}"
            );
        }
    }

    #[test]
    fn error_displays_message() {
        let cases = [
            (ParseCommentlessSymbolError::EndOfInput, "expected a symbol"),
            (
                ParseCommentlessSymbolError::InvalidCharacter('é'),
                "unexpected character `é`",
            ),
            (
                ParseCommentlessSymbolError::NotCommentless('@'),
                "unexpected character `@`",
            ),
            (
                ParseCommentlessSymbolError::UnmatchedEndCommentSymbol,
                "`*)` without a matching `(*`",
            ),
            (
                ParseCommentlessSymbolError::Integer(
                    ParseIntegerError::TooLarge,
                ),
                "integer is too large to fit in a u32",
            ),
            (
                ParseCommentlessSymbolError::TerminalString(
                    ParseTerminalStringError::Empty,
                ),
                "terminal string is empty",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn says_itself_what_a_wrapped_error_says() {
        let error = ParseCommentlessSymbolError::SpecialSequence(
            ParseSpecialSequenceError::Unsupported,
        );
        assert_eq!(
            error.to_string(),
            ParseSpecialSequenceError::Unsupported.to_string()
        );
        assert!(error.source().is_none());
    }

    #[test]
    fn tells_how_to_write_what_other_notations_write() {
        let help = |c| ParseCommentlessSymbolError::NotCommentless(c).help();
        assert_eq!(
            help(':').as_deref(),
            Some("write `=` to define a rule, as in `a = 'x';`")
        );
        assert_eq!(
            help('@').as_deref(),
            Some("write it in quotes to match it, as in `'@'`")
        );
        let help = |c| ParseCommentlessSymbolError::InvalidCharacter(c).help();
        assert_eq!(
            help('€').as_deref(),
            Some(
                "write it as `? U+20AC ?` to match it, since a special \
                 sequence can hold any character"
            )
        );
        assert_eq!(
            help('é').as_deref(),
            Some(
                "write the rule name without `é`, since one only holds ASCII \
                 letters, digits and spaces; to match `é`, write `? U+00E9 ?`"
            )
        );
    }
}
