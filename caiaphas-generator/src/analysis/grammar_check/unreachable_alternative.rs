//! An unreachable alternative comes after one that takes every match it could
//! have, such as `'if'` in `word = name | 'if';`, so its variant is never
//! built.
//!
//! An earlier alternative does when it matches every text the later one does,
//! since the first written of alternatives that tie wins: when it matches
//! every single character the later one does, or each of its few fixed texts,
//! and in a rule that takes the first match, when both are written the same
//! way. A greedy rule that takes the first match takes the earlier one
//! wherever it matches at all, so there it also does when it always matches,
//! when the later one starts with all of it, or when it is one fixed text that
//! every text of the later one starts with.

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
    // check, outside a rule that takes the first match.
    if earlier.is_written_like(later) {
        return rule.first_match;
    }
    let every_character = matches!(
        (facts.single_characters(earlier), facts.single_characters(later)),
        (Some(earlier), Some(later)) if later.is_subset(&earlier)
    );
    // A greedy rule that takes the first match takes the earlier alternative
    // wherever it matches at all.
    let taken_first = rule.first_match
        && rule.greedy
        && (facts.always_matches(earlier)
            || later.rest_after(earlier).is_some()
            || starts_every_text(grammar, earlier, later));
    every_character
        || has_every_text(grammar, rule.greedy, earlier, later)
        || taken_first
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

/// Returns whether `earlier` only ever matches one fixed text that every text
/// `later` can match starts with.
///
/// More texts would not do, since a sequence that takes as much as it can may
/// read past the one that fits and then fail.
fn starts_every_text(
    grammar: &Grammar,
    earlier: &Expression,
    later: &Expression,
) -> bool {
    let earlier_texts = matcher::texts(grammar, earlier);
    let Some([start]) = earlier_texts.as_deref() else {
        return false;
    };
    matcher::texts(grammar, later).is_some_and(|texts| {
        texts.iter().all(|text| text.starts_with(start.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{
            finding::{Finding, FindingCode},
            tests::{findings, greedy_findings},
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

    fn unreachable_when_greedy(source: &str) -> Vec<(Span, Span)> {
        spans(greedy_findings(source))
    }

    fn span(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    #[test]
    fn finds_every_alternative_after_one_that_always_matches_when_greedy() {
        let source = "(* match first *) a = 'z' | ['y'] | 'x' | 'w';";
        assert_eq!(
            unreachable_when_greedy(source),
            [(span(36, 39), span(28, 33)), (span(42, 45), span(28, 33))]
        );
        assert_eq!(unreachable(source), []);
    }

    #[test]
    fn finds_an_alternative_that_starts_with_an_earlier_one_when_greedy() {
        assert_eq!(
            unreachable_when_greedy("(* match first *) a = 'x' | 'xy';"),
            [(span(28, 32), span(22, 25))]
        );
        assert_eq!(
            unreachable_when_greedy("(* match first *) a = 'x' | 'x', 'y';"),
            [(span(28, 36), span(22, 25))]
        );
        assert_eq!(unreachable("(* match first *) a = 'x' | 'xy';"), []);
    }

    #[test]
    fn finds_an_alternative_written_like_an_earlier_one() {
        assert_eq!(
            unreachable("(* match first *) a = 'x' | 'x';"),
            [(span(28, 31), span(22, 25))]
        );
    }

    #[test]
    fn finds_an_alternative_whose_texts_an_earlier_one_matches() {
        assert_eq!(
            unreachable("(* match first *) a = ('ab' | 'cd') | 'cd';"),
            [(span(38, 42), span(22, 35))]
        );
    }

    #[test]
    fn finds_an_alternative_whose_characters_an_earlier_one_matches() {
        assert_eq!(
            unreachable("(* match first *) a = ? U+0061..U+007A ? | 'x';"),
            [(span(43, 46), span(22, 40))]
        );
    }

    #[test]
    fn accepts_alternatives_that_each_match_something_of_their_own() {
        assert_eq!(unreachable("(* match first *) a = 'xy' | 'x';"), []);
        assert_eq!(unreachable("(* match first *) a = 'x' | ['y'];"), []);
    }

    #[test]
    fn accepts_an_earlier_alternative_whose_exception_can_take_its_match() {
        assert_eq!(
            unreachable("(* match first *) a = ['y'] - ['z'] | 'x';"),
            []
        );
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
