//! Expectations mark a finding in a syntax rule as intended, so the analysis
//! does not report it.
//!
//! An expectation is a comment right before the rule, holding `expect`, then
//! the code of the finding in kebab case, then a colon and the reason:
//!
//! ```text
//! (* expect repeated-follow-conflict: the body of an abstraction extends as
//!    far right as it can *)
//! application = atom, {' ', argument};
//! ```
//!
//! Each comment holds one expectation, and a rule can have several. Being
//! comments, they mean nothing to other EBNF tools. The standard reads quotes
//! in a comment as terminal strings, so a reason cannot hold a lone `'` or
//! `"`.

use crate::span::Span;

/// An expectation that a syntax rule has a finding with a given code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expectation {
    /// The code of the finding, in kebab case.
    pub(crate) code: String,
    /// Why the finding is intended.
    pub(crate) reason: String,
    /// The comment it is written in.
    pub(crate) span: Span,
}

/// What a comment holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Comment {
    /// Holds text for people only.
    Ordinary,
    Expectation(Expectation),
    /// Text that starts with `expect` but is not written as an expectation.
    Malformed,
}

impl Comment {
    /// Reads the comment `text`, `(*` and `*)` included, found at `span`.
    pub(crate) fn read(text: &str, span: Span) -> Self {
        let content = text
            .strip_prefix("(*")
            .and_then(|text| text.strip_suffix("*)"))
            .unwrap_or(text);
        let Some(rest) =
            content.trim_start_matches(is_gap).strip_prefix("expect")
        else {
            return Self::Ordinary;
        };
        if rest.chars().next().is_some_and(|c| !is_gap(c) && c != ':') {
            return Self::Ordinary;
        }
        let Some((code, reason)) = rest.split_once(':') else {
            return Self::Malformed;
        };
        let code = code.trim_matches(is_gap);
        let reason = reason.trim_matches(is_gap);
        if !is_kebab_case(code) || reason.is_empty() {
            return Self::Malformed;
        }
        Self::Expectation(Expectation {
            code: code.to_owned(),
            reason: reason.to_owned(),
            span,
        })
    }
}

fn is_gap(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C')
}

/// Returns whether `text` is lowercase words joined by `-`.
fn is_kebab_case(text: &str) -> bool {
    text.split('-').all(|word| {
        !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Comment {
        Comment::read(
            text,
            Span {
                start: 0,
                end: text.len(),
            },
        )
    }

    fn expectation(code: &str, reason: &str, length: usize) -> Comment {
        Comment::Expectation(Expectation {
            code: code.to_owned(),
            reason: reason.to_owned(),
            span: Span {
                start: 0,
                end: length,
            },
        })
    }

    #[test]
    fn reads_an_expectation() {
        let text = "(* expect shadowed-branch: the first is meant *)";
        assert_eq!(
            read(text),
            expectation("shadowed-branch", "the first is meant", text.len())
        );
    }

    #[test]
    fn reads_an_expectation_over_several_lines() {
        let text = "(*\n  expect empty-rule :\n  kept for later\n*)";
        assert_eq!(
            read(text),
            expectation("empty-rule", "kept for later", text.len())
        );
    }

    #[test]
    fn keeps_colons_in_the_reason() {
        let text = "(* expect empty-rule: see: below *)";
        assert_eq!(
            read(text),
            expectation("empty-rule", "see: below", text.len())
        );
    }

    #[test]
    fn reads_other_comments_as_ordinary() {
        for text in [
            "(* a comment *)",
            "(**)",
            "(* expected to change *)",
            "(* expectation: none *)",
            "(* see expect below *)",
        ] {
            assert_eq!(read(text), Comment::Ordinary, "{text:?}");
        }
    }

    #[test]
    fn rejects_an_expectation_written_wrongly() {
        for text in [
            "(* expect *)",
            "(* expect shadowed-branch *)",
            "(* expect shadowed-branch: *)",
            "(* expect : no code *)",
            "(* expect: no code *)",
            "(* expect Shadowed-Branch: case *)",
            "(* expect shadowed_branch: underscore *)",
            "(* expect shadowed--branch: empty word *)",
            "(* expect shadowed branch: space *)",
        ] {
            assert_eq!(read(text), Comment::Malformed, "{text:?}");
        }
    }
}
