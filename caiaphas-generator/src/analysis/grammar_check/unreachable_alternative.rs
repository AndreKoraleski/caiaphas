//! An unreachable alternative comes after one that takes every match it could
//! have, in a rule that takes the first match, such as the second `'x'` in
//! `(* first match *) a = 'x' | 'x';`.
//!
//! An earlier alternative does when it matches every text the later one does:
//! when both are written the same way, when it matches every single character
//! the later one does, or every one of the few fixed texts. A greedy rule
//! takes the earlier one wherever it matches at all, so there it also does
//! when it always matches, when the later one starts with all of it, or when
//! it is one fixed text that every text of the later one starts with.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
        grammar_check::longer_branch_conflict::rest_after,
        matcher,
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar},
};

/// Reports every alternative of a rule that takes the first match that comes
/// after one that matches wherever it would.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let first_match =
        grammar.syntax_rules.iter().filter(|rule| rule.first_match);
    for rule in first_match {
        rule.expression.visit(&mut |expression| {
            let ExpressionKind::DefinitionsList(alternatives) =
                &expression.kind
            else {
                return;
            };
            for (index, later) in alternatives.iter().enumerate() {
                let earlier = alternatives[..index].iter().find(|earlier| {
                    covers(grammar, facts, rule.greedy, earlier, later)
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

/// Returns whether `earlier` takes every match `later` could have, in a rule
/// that is `greedy` or not.
fn covers(
    grammar: &Grammar,
    facts: &Facts<'_>,
    greedy: bool,
    earlier: &Expression,
    later: &Expression,
) -> bool {
    let single_characters = (
        facts.single_characters(earlier),
        facts.single_characters(later),
    );
    let every_text = earlier.is_written_like(later)
        || matches!(
            single_characters,
            (Some(earlier), Some(later)) if later.is_subset(&earlier)
        )
        || has_every_text(grammar, earlier, later);
    every_text
        || greedy
            && (facts.always_matches(earlier)
                || rest_after(earlier, later).is_some()
                || starts_every_text(grammar, earlier, later))
}

/// Returns whether `earlier` matches every one of the few fixed texts `later`
/// can match.
fn has_every_text(
    grammar: &Grammar,
    earlier: &Expression,
    later: &Expression,
) -> bool {
    let Some(earlier) = matcher::texts(grammar, earlier) else {
        return false;
    };
    matcher::texts(grammar, later)
        .is_some_and(|texts| texts.iter().all(|text| earlier.contains(text)))
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
        let source = "(* first match *) a = 'z' | ['y'] | 'x' | 'w';";
        assert_eq!(
            unreachable_when_greedy(source),
            [(span(36, 39), span(28, 33)), (span(42, 45), span(28, 33))]
        );
        assert_eq!(unreachable(source), []);
    }

    #[test]
    fn finds_an_alternative_that_starts_with_an_earlier_one_when_greedy() {
        assert_eq!(
            unreachable_when_greedy("(* first match *) a = 'x' | 'xy';"),
            [(span(28, 32), span(22, 25))]
        );
        assert_eq!(
            unreachable_when_greedy("(* first match *) a = 'x' | 'x', 'y';"),
            [(span(28, 36), span(22, 25))]
        );
        assert_eq!(unreachable("(* first match *) a = 'x' | 'xy';"), []);
    }

    #[test]
    fn finds_an_alternative_written_like_an_earlier_one() {
        assert_eq!(
            unreachable("(* first match *) a = 'x' | 'x';"),
            [(span(28, 31), span(22, 25))]
        );
    }

    #[test]
    fn finds_an_alternative_whose_texts_an_earlier_one_matches() {
        assert_eq!(
            unreachable("(* first match *) a = ('ab' | 'cd') | 'cd';"),
            [(span(38, 42), span(22, 35))]
        );
    }

    #[test]
    fn finds_an_alternative_whose_characters_an_earlier_one_matches() {
        assert_eq!(
            unreachable("(* first match *) a = ? U+0061..U+007A ? | 'x';"),
            [(span(43, 46), span(22, 40))]
        );
    }

    #[test]
    fn accepts_alternatives_that_each_match_something_of_their_own() {
        assert_eq!(unreachable("(* first match *) a = 'xy' | 'x';"), []);
        assert_eq!(unreachable("(* first match *) a = 'x' | ['y'];"), []);
    }

    #[test]
    fn accepts_an_earlier_alternative_whose_exception_can_take_its_match() {
        assert_eq!(
            unreachable("(* first match *) a = ['y'] - ['z'] | 'x';"),
            []
        );
    }

    #[test]
    fn leaves_rules_that_take_the_longest_match_alone() {
        assert_eq!(unreachable("a = ['y'] | 'x' | 'xy';"), []);
    }
}
