//! An unreachable alternative comes after one that takes every match it could
//! have, such as `'if'` in `word = name | 'if';`, so its variant is never
//! built.
//!
//! An earlier alternative does when it matches every text the later one does,
//! since the first written of alternatives that tie wins: when it matches
//! every single character the later one does, or each of its few fixed texts.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
        matcher,
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar, Rule},
};

/// Reports every alternative that comes after one that takes every match it
/// could have.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for rule in &grammar.syntax_rules {
        rule.expression.visit(&mut |expression| {
            let ExpressionKind::DefinitionsList(alternatives) =
                &expression.kind
            else {
                return;
            };
            for (index, later) in alternatives.iter().enumerate() {
                let earlier = alternatives[..index].iter().find(|earlier| {
                    covers(grammar, facts, rule, earlier, later)
                });
                if let Some(earlier) = earlier {
                    findings.push(Finding {
                        kind: FindingKind::UnreachableAlternative,
                        span: later.span,
                        labels: vec![Label {
                            span: earlier.span,
                            message: "this alternative matches first",
                        }],
                    });
                }
            }
        });
    }
}

/// Returns whether `earlier` takes every match `later` could have, in `rule`.
pub(super) fn covers(
    grammar: &Grammar,
    facts: &Facts<'_>,
    rule: &Rule,
    earlier: &Expression,
    later: &Expression,
) -> bool {
    // Alternatives written the same way are left to the duplicate alternative
    // check.
    if earlier.is_written_like(later) {
        return false;
    }
    let every_character = matches!(
        (facts.single_characters(earlier), facts.single_characters(later)),
        (Some(earlier), Some(later)) if later.is_subset(&earlier)
    );
    every_character || has_every_text(grammar, rule.greedy, earlier, later)
}

/// Returns whether `earlier`, written in a rule that is `greedy` or not,
/// matches all of each of the few fixed texts `later` can match.
fn has_every_text(
    grammar: &Grammar,
    greedy: bool,
    earlier: &Expression,
    later: &Expression,
) -> bool {
    matcher::texts(grammar, later).is_some_and(|texts| {
        texts.iter().all(|text| {
            matcher::matches_all(grammar, earlier, text, greedy) == Some(true)
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{
            finding::{Finding, FindingCode},
            tests::findings,
        },
        span::Span,
    };

    fn spans(found: Vec<Finding>) -> Vec<(Span, Span)> {
        found
            .into_iter()
            .filter(|finding| {
                finding.kind.code() == FindingCode::UnreachableAlternative
            })
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    fn unreachable(source: &str) -> Vec<(Span, Span)> {
        spans(findings(source))
    }

    fn span(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    #[test]
    fn finds_an_alternative_whose_texts_an_earlier_one_matches() {
        assert_eq!(
            unreachable("a = ('ab' | 'cd') | 'cd';"),
            [(span(20, 24), span(4, 17))]
        );
    }

    #[test]
    fn finds_an_alternative_whose_characters_an_earlier_one_matches() {
        assert_eq!(
            unreachable("a = ? U+0061..U+007A ? | 'x';"),
            [(span(25, 28), span(4, 22))]
        );
    }

    #[test]
    fn accepts_alternatives_that_each_match_something_of_their_own() {
        assert_eq!(unreachable("a = 'xy' | 'x';"), []);
        assert_eq!(unreachable("a = 'x' | ['y'];"), []);
    }

    #[test]
    fn accepts_an_earlier_alternative_whose_exception_can_take_its_match() {
        assert_eq!(unreachable("a = ['y'] - ['z'] | 'x';"), []);
    }

    #[test]
    fn finds_a_word_that_an_earlier_name_matches_in_any_rule() {
        let source = "word = name | 'if'; name = letter, {letter}; \
                      letter = ? U+0061..U+007A ?;";
        assert_eq!(unreachable(source), [(span(14, 18), span(7, 11))]);
        assert_eq!(
            unreachable("a = 'x', {'x'} | 'xx';"),
            [(span(17, 21), span(4, 14))]
        );
    }

    #[test]
    fn leaves_alternatives_that_match_more_or_are_written_alike() {
        assert_eq!(unreachable("a = ['y'] | 'x' | 'xy';"), []);
        assert_eq!(unreachable("a = 'x' | 'x';"), []);
    }
}
