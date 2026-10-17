//! Meta identifiers of ISO/IEC 14977 EBNF.

use crate::lexer::{gap_separator, terminal_character::TerminalCharacter};

/// A meta identifier names a rule.
///
/// It is a letter followed by any number of letters and digits. Gaps may come
/// between those characters but are not part of the name, so `syntax rule` and
/// `syntaxrule` are the same meta identifier.
#[derive(Debug, Clone)]
pub(crate) struct MetaIdentifier {
    name: String,
    /// The meta identifier as written, gaps included, to tell where its words
    /// start.
    written: String,
}

impl MetaIdentifier {
    /// Returns the meta identifier at the start of `input`, and how long it
    /// is, gaps between its characters included.
    pub(crate) fn read(input: &str) -> Option<(Self, usize)> {
        if !input.chars().next().is_some_and(is_letter) {
            return None;
        }
        let (name, length) =
            gap_separator::remove(input, is_meta_identifier_character);
        let written = input[..length].to_owned();
        Some((Self { name, written }, length))
    }

    /// Returns the name, without gaps.
    pub(crate) fn as_str(&self) -> &str {
        &self.name
    }

    /// Returns the meta identifier as written, gaps included.
    pub(crate) fn written(&self) -> &str {
        &self.written
    }

    /// Returns the meta identifier with single spaces between its words, as a
    /// message shows it.
    pub(crate) fn spaced(&self) -> String {
        self.words().collect::<Vec<_>>().join(" ")
    }

    /// Returns the words that the gaps in the meta identifier separate.
    pub(crate) fn words(&self) -> impl Iterator<Item = &str> {
        self.written
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
    }
}

impl PartialEq for MetaIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for MetaIdentifier {}

fn is_letter(c: char) -> bool {
    matches!(
        TerminalCharacter::read(c, None),
        Some((TerminalCharacter::Letter, _))
    )
}

/// Returns whether `c` is a letter or a decimal digit.
fn is_meta_identifier_character(c: char) -> bool {
    matches!(
        TerminalCharacter::read(c, None),
        Some((
            TerminalCharacter::Letter | TerminalCharacter::DecimalDigit,
            _
        ))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(input: &str) -> Option<(String, usize)> {
        MetaIdentifier::read(input).map(|(identifier, length)| {
            (identifier.as_str().to_owned(), length)
        })
    }

    #[test]
    fn reads_letters_and_digits() {
        assert_eq!(read("a"), Some(("a".to_owned(), 1)));
        assert_eq!(read("rule2b"), Some(("rule2b".to_owned(), 6)));
    }

    #[test]
    fn keeps_case() {
        assert_eq!(read("Rule"), Some(("Rule".to_owned(), 4)));
        assert_ne!(
            MetaIdentifier::read("rule").map(|(identifier, _)| identifier),
            MetaIdentifier::read("Rule").map(|(identifier, _)| identifier)
        );
    }

    #[test]
    fn removes_gaps_between_characters() {
        assert_eq!(read("syntax rule"), Some(("syntaxrule".to_owned(), 11)));
        assert_eq!(
            read("decimal\r\n digit"),
            Some(("decimaldigit".to_owned(), 15))
        );
        assert_eq!(
            MetaIdentifier::read("syntax rule")
                .map(|(identifier, _)| identifier),
            MetaIdentifier::read("syntaxrule")
                .map(|(identifier, _)| identifier)
        );
    }

    #[test]
    fn keeps_the_words_the_gaps_separate() {
        let words = |input| {
            let (identifier, _) = MetaIdentifier::read(input).unwrap();
            identifier.words().map(str::to_owned).collect::<Vec<_>>()
        };
        assert_eq!(words("syntax rule"), ["syntax", "rule"]);
        assert_eq!(words("decimal\r\n digit 2"), ["decimal", "digit", "2"]);
        assert_eq!(words("syntaxRule = x"), ["syntaxRule"]);
    }

    #[test]
    fn leaves_a_gap_after_the_last_character() {
        assert_eq!(read("rule = 'a';"), Some(("rule".to_owned(), 4)));
        assert_eq!(read("rule \n"), Some(("rule".to_owned(), 4)));
    }

    #[test]
    fn stops_at_a_character_that_is_not_a_letter_or_digit() {
        for input in ["a,b", "a=b", "a_b", "a-b", "a(*b*)", "a'b'", "aé"] {
            assert_eq!(read(input), Some(("a".to_owned(), 1)), "{input:?}");
        }
    }

    #[test]
    fn rejects_text_that_does_not_start_with_a_letter() {
        for input in ["", "1a", "_a", " a", "'a'", "éa"] {
            assert_eq!(read(input), None, "{input:?}");
        }
    }
}
