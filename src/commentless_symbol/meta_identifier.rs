//! Meta identifiers of ISO/IEC 14977 EBNF.

use std::{error::Error, fmt};

use crate::{gap_separator, terminal_character::TerminalCharacter};

/// A meta identifier, which names a rule.
///
/// It is a letter followed by any number of letters and digits. Gaps may come
/// between those characters but are not part of the name, so `syntax rule` and
/// `syntaxrule` are the same meta identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MetaIdentifier(String);

impl MetaIdentifier {
    /// Returns the meta identifier at the start of `input`, and how long it
    /// is, gaps between its characters included.
    pub(crate) fn read(
        input: &str,
    ) -> Result<(Self, usize), ParseMetaIdentifierError> {
        if !input.chars().next().is_some_and(is_letter) {
            return Err(ParseMetaIdentifierError);
        }
        let (name, length) =
            gap_separator::remove(input, is_meta_identifier_character);
        Ok((Self(name), length))
    }

    /// Returns the name, without gaps.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_letter(c: char) -> bool {
    matches!(
        TerminalCharacter::read(c, None),
        Ok((TerminalCharacter::Letter(_), _))
    )
}

/// Returns whether `c` is a meta identifier character, a letter or a decimal
/// digit.
fn is_meta_identifier_character(c: char) -> bool {
    matches!(
        TerminalCharacter::read(c, None),
        Ok((
            TerminalCharacter::Letter(_) | TerminalCharacter::DecimalDigit(_),
            _
        ))
    )
}

/// Error returned when text does not start with a [`MetaIdentifier`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParseMetaIdentifierError;

impl fmt::Display for ParseMetaIdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EBNF meta identifier does not start with a letter")
    }
}

impl Error for ParseMetaIdentifierError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(input: &str) -> Result<(String, usize), ParseMetaIdentifierError> {
        MetaIdentifier::read(input).map(|(identifier, length)| {
            (identifier.as_str().to_owned(), length)
        })
    }

    #[test]
    fn reads_letters_and_digits() {
        assert_eq!(read("a"), Ok(("a".to_owned(), 1)));
        assert_eq!(read("rule2b"), Ok(("rule2b".to_owned(), 6)));
    }

    #[test]
    fn keeps_case() {
        assert_eq!(read("Rule"), Ok(("Rule".to_owned(), 4)));
        assert_ne!(
            MetaIdentifier::read("rule").map(|(identifier, _)| identifier),
            MetaIdentifier::read("Rule").map(|(identifier, _)| identifier)
        );
    }

    #[test]
    fn removes_gaps_between_characters() {
        assert_eq!(read("syntax rule"), Ok(("syntaxrule".to_owned(), 11)));
        assert_eq!(
            read("decimal\r\n digit"),
            Ok(("decimaldigit".to_owned(), 15))
        );
        assert_eq!(
            MetaIdentifier::read("syntax rule")
                .map(|(identifier, _)| identifier),
            MetaIdentifier::read("syntaxrule")
                .map(|(identifier, _)| identifier)
        );
    }

    #[test]
    fn leaves_a_gap_after_the_last_character() {
        assert_eq!(read("rule = 'a';"), Ok(("rule".to_owned(), 4)));
        assert_eq!(read("rule \n"), Ok(("rule".to_owned(), 4)));
    }

    #[test]
    fn stops_at_a_character_that_is_not_a_letter_or_digit() {
        for input in ["a,b", "a=b", "a_b", "a-b", "a(*b*)", "a'b'", "aé"] {
            assert_eq!(read(input), Ok(("a".to_owned(), 1)), "{input:?}");
        }
    }

    #[test]
    fn rejects_text_that_does_not_start_with_a_letter() {
        for input in ["", "1a", "_a", " a", "'a'", "éa"] {
            assert_eq!(
                read(input),
                Err(ParseMetaIdentifierError),
                "{input:?}"
            );
        }
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            ParseMetaIdentifierError.to_string(),
            "EBNF meta identifier does not start with a letter"
        );
    }

    #[test]
    fn error_implements_error_trait() {
        let error: &dyn Error = &ParseMetaIdentifierError;
        assert!(error.source().is_none());
    }
}
