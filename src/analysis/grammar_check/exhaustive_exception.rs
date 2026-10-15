//! An exhaustive syntactic exception removes everything its factor matches,
//! such as `letter` in `a = letter - letter;`, so the syntactic term can never
//! match.
//!
//! It is the opposite of an exception that can never apply. It can only be
//! told exactly when both sides only ever match one character, such as
//! terminal strings of one character, special sequences and alternatives of
//! those, so other terms are left alone.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind, Label},
    },
    ir::{ExpressionKind, Grammar},
};

/// Reports every syntactic term whose exception matches every character its
/// factor does.
pub(super) fn check(
    _grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    facts.walk(&mut |expression, _| {
        let ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } = &expression.kind
        else {
            return;
        };
        let (Some(factor), Some(exception)) = (
            facts.single_characters(syntactic_factor),
            facts.single_characters(syntactic_exception),
        ) else {
            return;
        };
        if factor.is_subset(&exception) {
            findings.push(Finding {
                kind: FindingKind::ExhaustiveException,
                span: expression.span,
                labels: vec![Label {
                    span: syntactic_exception.span,
                    message: "this matches every character before the `-`",
                }],
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        span::Span,
    };

    fn exhaustive(source: &str) -> Vec<(Span, Span)> {
        findings(source)
            .into_iter()
            .filter(|finding| finding.kind == FindingKind::ExhaustiveException)
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    #[test]
    fn finds_an_exception_that_matches_everything_its_factor_does() {
        assert_eq!(
            exhaustive("a = 'x' - ? U+0061..U+007A ?;"),
            [(Span { start: 4, end: 28 }, Span { start: 10, end: 28 })]
        );
    }

    #[test]
    fn finds_it_through_rules_and_alternatives() {
        let source = "s = letter - (vowel | consonant); \
                      letter = ? U+0061..U+007A ?; \
                      vowel = 'a' | 'e' | 'i' | 'o' | 'u'; \
                      consonant = letter - vowel;";
        assert_eq!(exhaustive(source).len(), 1);
    }

    #[test]
    fn accepts_an_exception_that_leaves_some_characters() {
        assert_eq!(exhaustive("a = ? U+0061..U+007A ? - 'x';"), []);
    }

    #[test]
    fn leaves_terms_that_match_more_than_one_character() {
        assert_eq!(exhaustive("a = 'xy' - 'xy';"), []);
        assert_eq!(exhaustive("a = 'x' - ['x'];"), []);
    }
}
