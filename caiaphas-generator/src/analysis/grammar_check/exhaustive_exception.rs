//! An exhaustive syntactic exception removes everything its factor matches,
//! such as `letter` in `a = letter - letter;`, so the syntactic term can never
//! match.
//!
//! It is the opposite of an exception that can never apply. It can be told
//! when both sides only ever match one character, or when the factor matches a
//! few fixed texts, so other terms are left alone.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
        matcher,
    },
    diagnostic::Label,
    ir::{ExpressionKind, Grammar},
};

/// Reports every syntactic term whose exception matches every character its
/// factor does.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    grammar.visit(&mut |expression| {
        let ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } = &expression.kind
        else {
            return;
        };
        let characters = match (
            facts.single_characters(syntactic_factor),
            facts.single_characters(syntactic_exception),
        ) {
            (Some(factor), Some(exception)) => factor.is_subset(&exception),
            _ => false,
        };
        let texts =
            matcher::texts(grammar, syntactic_factor).is_some_and(|texts| {
                !texts.is_empty()
                    && texts.iter().all(|text| {
                        matcher::excludes(grammar, syntactic_exception, text)
                            == Some(true)
                    })
            });
        if characters || texts {
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
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn exhaustive(source: &str) -> Vec<(Span, Span)> {
        findings_with(source, FindingCode::ExhaustiveException)
            .into_iter()
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
    fn finds_an_exception_that_matches_every_text_of_its_factor() {
        assert_eq!(exhaustive("a = 'xy' - 'xy';").len(), 1);
        assert_eq!(exhaustive("a = 'x' - {'x'};").len(), 1);
        assert_eq!(exhaustive("a = 'ab' - ('a', 'b');").len(), 1);
    }

    #[test]
    fn leaves_a_factor_of_texts_without_end_alone() {
        assert_eq!(exhaustive("a = {'x'} - 'x';"), []);
    }
}
