//! A longer branch conflict is an alternative that goes on from where another
//! alternative of the same choice ends, with characters that can also come
//! after the choice, such as `'x', 'y'` in `s = ('x' | 'x', 'y'), 'y';`.
//!
//! A greedy rule takes the alternative that matches the most, so it takes the
//! longer one whenever it can, even when what comes after the choice needed
//! the shorter one. Every other rule tries both, so only a greedy rule reports
//! it. It is the follow conflict of an optional sequence, since `a | a, b`
//! matches like `a, [b]`. A rule that takes the first match never takes a
//! longer alternative written after the shorter one, so only one written
//! before it counts there.

use crate::{
    analysis::{
        character_set::CharacterSet,
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar, RuleId},
};

/// Reports every alternative of a greedy rule that goes on from where another
/// alternative of the same definitions list ends, with characters that can
/// come after the list.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        if !rule.greedy {
            continue;
        }
        facts.walk_rule(RuleId(position), &mut |expression, after| {
            let ExpressionKind::DefinitionsList(alternatives) =
                &expression.kind
            else {
                return;
            };
            for (index, longer) in alternatives.iter().enumerate() {
                let shorter_ones = if rule.first_match {
                    &alternatives[index + 1..]
                } else {
                    alternatives.as_slice()
                };
                let conflict = shorter_ones.iter().find_map(|shorter| {
                    let rest = rest_after(shorter, longer)?;
                    let characters = rest
                        .iter()
                        .rev()
                        .fold(CharacterSet::default(), |after, expression| {
                            facts.first_before(expression, &after)
                        })
                        .intersection(&after.characters);
                    (!characters.is_empty()).then_some((shorter, characters))
                });
                if let Some((shorter, characters)) = conflict {
                    let label = Label {
                        span: shorter.span,
                        message: "this branch ends where the longer one goes \
                                  on",
                    };
                    findings.push(Finding {
                        kind: FindingKind::LongerBranchConflict { characters },
                        span: longer.span,
                        labels: vec![label],
                    });
                }
            }
        });
    }
}

/// Returns what `longer` goes on with after `shorter`, when `longer` starts
/// with every expression of `shorter` and has more.
pub(super) fn rest_after<'e>(
    shorter: &Expression,
    longer: &'e Expression,
) -> Option<&'e [Expression]> {
    let (shorter, longer) = (shorter.steps(), longer.steps());
    let starts_alike = shorter
        .iter()
        .zip(longer)
        .all(|(shorter, longer)| shorter.is_written_like(longer));
    (starts_alike && longer.len() > shorter.len())
        .then(|| &longer[shorter.len()..])
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::greedy_findings},
        span::Span,
    };

    fn conflicts(source: &str) -> Vec<(String, Span, Span)> {
        greedy_findings(source)
            .into_iter()
            .filter(|finding| {
                finding.kind.code() == FindingCode::LongerBranchConflict
            })
            .map(|finding| {
                (
                    finding.kind.to_string(),
                    finding.span,
                    finding.labels[0].span,
                )
            })
            .collect()
    }

    #[test]
    fn finds_a_branch_that_goes_on_like_what_follows_the_choice() {
        assert_eq!(
            conflicts("s = ('x' | 'x', 'y'), 'y';"),
            [(
                "this branch, past where another ends, and what comes after \
                 the choice can both start with `y`, so a parser cannot tell \
                 which to take"
                    .to_owned(),
                Span { start: 11, end: 19 },
                Span { start: 5, end: 8 },
            )]
        );
    }

    #[test]
    fn finds_a_longer_branch_a_first_match_takes_before_the_shorter() {
        let first_match = "(* first match *) s = ('x', 'y' | 'x'), 'y';";
        assert_eq!(conflicts(first_match).len(), 1);
        assert_eq!(
            conflicts("(* first match *) s = ('x' | 'x', 'y'), 'y';"),
            []
        );
    }

    #[test]
    fn finds_what_follows_through_other_rules() {
        let found = conflicts("s = a, 'y'; a = b | b, 'y'; b = 'x';");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, Span { start: 20, end: 26 });
    }

    #[test]
    fn accepts_a_branch_that_goes_on_differently() {
        assert_eq!(conflicts("s = ('x' | 'x', 'y'), 'z';"), []);
        assert_eq!(conflicts("s = 'x' | 'x', 'y';"), []);
    }

    #[test]
    fn accepts_branches_that_only_start_alike() {
        assert_eq!(conflicts("s = ('x', 'y' | 'x', 'z'), 'z';"), []);
    }
}
