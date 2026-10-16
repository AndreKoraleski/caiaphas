//! Annotations are comments right before a syntax rule that tell caiaphas
//! something about the rule. Being comments, they mean nothing to other EBNF
//! tools.
//!
//! An expectation marks a finding in the rule as intended, so the analysis
//! does not report it. It holds `expect`, then the code of the finding in
//! kebab case, then a colon and the reason:
//!
//! ```text
//! (* expect tied-branches: a keyword is never a name *)
//! word = keyword | name;
//! ```
//!
//! A hide, written `(* hide *)`, leaves the rule out of the types generated
//! for the grammar, so the rules that use it get no field for it. It suits
//! rules whose matches nobody reads, such as whitespace between tokens.
//!
//! A first match, written `(* first match *)`, makes every choice written in
//! the rule take the first alternative after which the rest of the input still
//! matches, in the order written, instead of the one that matches the most. It
//! suits rules whose order says which alternative wins, as it does in a PEG.
//!
//! A greedy, written `(* greedy *)`, makes the rule take each way through it
//! as it comes to it, even where the rest of the input needs another, so its
//! parser never tries another way. It suits rules where taking the most at
//! each step is what the grammar means, such as the `else` of a nested `if`,
//! and keeps their parser fast.
//!
//! An inline, written `(* inline *)`, gives the rule no type of its own. Each
//! rule that holds it holds what it holds instead, as if its definition were
//! written there, so a rule holding `group` below holds its `expression`:
//!
//! ```text
//! (* inline *)
//! group = '(', expression, ')';
//! ```
//!
//! Each comment holds one annotation, and a rule can have several. The
//! standard reads quotes in a comment as terminal strings, so a reason cannot
//! hold a lone `'` or `"`.

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

/// What a comment right before a syntax rule tells caiaphas about the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Annotation {
    Expectation(Expectation),
    /// Leaves the rule out of the types generated for the grammar.
    Hide,
    /// Makes the choices written in the rule take their first alternative that
    /// matches.
    FirstMatch,
    /// Makes the rule take each way through it as it comes to it.
    Greedy,
    /// Gives the rules that hold the rule what it holds, instead of its type.
    Inline,
}

/// What a comment holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Comment {
    /// Holds text for people only.
    Ordinary,
    Annotation(Annotation),
    /// Text that starts with `expect` but is not written as an expectation.
    MalformedExpectation,
    /// Text that starts with `hide` but holds more than that.
    MalformedHide,
    /// Text that starts with `first match` but holds more than that.
    MalformedFirstMatch,
    /// Text that starts with `greedy` but holds more than that.
    MalformedGreedy,
    /// Text that starts with `inline` but holds more than that.
    MalformedInline,
}

impl Comment {
    /// Reads the comment `text`, `(*` and `*)` included, found at `span`.
    pub(crate) fn read(text: &str, span: Span) -> Self {
        let content = text
            .strip_prefix("(*")
            .and_then(|text| text.strip_suffix("*)"))
            .unwrap_or(text)
            .trim_matches(is_gap);
        if let Some(rest) = after_word(content, "hide") {
            return if rest.is_empty() {
                Self::Annotation(Annotation::Hide)
            } else {
                Self::MalformedHide
            };
        }
        if let Some(rest) = after_word(content, "inline") {
            return if rest.is_empty() {
                Self::Annotation(Annotation::Inline)
            } else {
                Self::MalformedInline
            };
        }
        if let Some(rest) = after_word(content, "greedy") {
            return if rest.is_empty() {
                Self::Annotation(Annotation::Greedy)
            } else {
                Self::MalformedGreedy
            };
        }
        let first_match = after_word(content, "first").and_then(|rest| {
            after_word(rest.trim_start_matches(is_gap), "match")
        });
        if let Some(rest) = first_match {
            return if rest.is_empty() {
                Self::Annotation(Annotation::FirstMatch)
            } else {
                Self::MalformedFirstMatch
            };
        }
        let Some(rest) = after_word(content, "expect") else {
            return Self::Ordinary;
        };
        let Some((code, reason)) = rest.split_once(':') else {
            return Self::MalformedExpectation;
        };
        let code = code.trim_matches(is_gap);
        let reason = reason.trim_matches(is_gap);
        if !is_kebab_case(code) || reason.is_empty() {
            return Self::MalformedExpectation;
        }
        Self::Annotation(Annotation::Expectation(Expectation {
            code: code.to_owned(),
            reason: reason.to_owned(),
            span,
        }))
    }
}

/// Returns what follows `word` at the start of `content` when it is a word of
/// its own there, so `expect` is found in `expect:` but not in `expected`.
fn after_word<'a>(content: &'a str, word: &str) -> Option<&'a str> {
    let rest = content.strip_prefix(word)?;
    rest.chars()
        .next()
        .is_none_or(|c| is_gap(c) || c == ':')
        .then_some(rest)
}

/// Returns whether `c` can be part of a gap. A lexer only lets a carriage
/// return into a comment as part of a new line, so taking one on its own here
/// changes nothing.
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
        Comment::Annotation(Annotation::Expectation(Expectation {
            code: code.to_owned(),
            reason: reason.to_owned(),
            span: Span {
                start: 0,
                end: length,
            },
        }))
    }

    #[test]
    fn reads_an_expectation() {
        let text = "(* expect tied-branches: the first is meant *)";
        assert_eq!(
            read(text),
            expectation("tied-branches", "the first is meant", text.len())
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
            "(* hidden for now *)",
            "(* hides the rest *)",
            "(* first, the header *)",
            "(* first matching rule *)",
        ] {
            assert_eq!(read(text), Comment::Ordinary, "{text:?}");
        }
    }

    #[test]
    fn reads_a_hide() {
        for text in ["(* hide *)", "(*hide*)", "(*\n  hide\n*)"] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Hide),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_a_hide_with_more_after_it() {
        for text in ["(* hide whitespace *)", "(* hide: whitespace *)"] {
            assert_eq!(read(text), Comment::MalformedHide, "{text:?}");
        }
    }

    #[test]
    fn reads_a_first_match() {
        for text in [
            "(* first match *)",
            "(*first  match*)",
            "(*\nfirst\nmatch\n*)",
        ] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::FirstMatch),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_a_first_match_with_more_after_it() {
        for text in ["(* first match wins *)", "(* first match: always *)"] {
            assert_eq!(read(text), Comment::MalformedFirstMatch, "{text:?}");
        }
    }

    #[test]
    fn reads_a_greedy() {
        for text in ["(* greedy *)", "(*greedy*)", "(*\n  greedy\n*)"] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Greedy),
                "{text:?}"
            );
        }
        assert_eq!(read("(* greedy else *)"), Comment::MalformedGreedy);
    }

    #[test]
    fn reads_an_inline() {
        for text in ["(* inline *)", "(*inline*)", "(*\n  inline\n*)"] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Inline),
                "{text:?}"
            );
        }
        assert_eq!(read("(* inline group *)"), Comment::MalformedInline);
    }

    #[test]
    fn rejects_an_expectation_written_wrongly() {
        for text in [
            "(* expect *)",
            "(* expect tied-branches *)",
            "(* expect tied-branches: *)",
            "(* expect : no code *)",
            "(* expect: no code *)",
            "(* expect Tied-Branches: case *)",
            "(* expect tied_branches: underscore *)",
            "(* expect tied--branches: empty word *)",
            "(* expect tied branches: space *)",
        ] {
            assert_eq!(read(text), Comment::MalformedExpectation, "{text:?}");
        }
    }
}
