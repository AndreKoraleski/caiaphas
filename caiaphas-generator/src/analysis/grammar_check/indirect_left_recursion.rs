//! Indirect left recursion makes a parser reach a rule again through other
//! rules before reading anything, such as `a = b, 'x' | 'y';` with
//! `b = a, 'z' | 'w';`, so it never ends.
//!
//! The left recursion check reports a rule that reaches itself directly. This
//! one reports the meta identifiers that lead back through others.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::{Grammar, RuleId, reachable},
    span::Span,
};

/// Reports every meta identifier a rule can read before any character that
/// leads back to the rule through other rules. Of the rules in one such cycle,
/// only the first written is reported, since fixing it fixes them all.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let first_meta_identifiers: Vec<Vec<(RuleId, Span)>> = grammar
        .syntax_rules
        .iter()
        .map(|rule| facts.first_meta_identifiers(&rule.expression))
        .collect();
    let in_cycle = |from: usize, to: usize| {
        reaches(&first_meta_identifiers, RuleId(from), RuleId(to))
            && reaches(&first_meta_identifiers, RuleId(to), RuleId(from))
    };
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        if (0..position).any(|earlier| in_cycle(earlier, position)) {
            continue;
        }
        for &(target, span) in &first_meta_identifiers[position] {
            if target != RuleId(position)
                && reaches(&first_meta_identifiers, target, RuleId(position))
            {
                let RuleId(through) = target;
                findings.push(Finding {
                    kind: FindingKind::IndirectLeftRecursion {
                        meta_identifier: rule.meta_identifier.clone(),
                        through: grammar.syntax_rules[through]
                            .meta_identifier
                            .clone(),
                    },
                    span,
                    labels: vec![Label {
                        span: rule.span,
                        message: "in this rule",
                    }],
                });
            }
        }
    }
}

/// Returns whether `to` can be read before any character, starting from `from`
/// and following the meta identifiers each rule can read first.
fn reaches(
    first_meta_identifiers: &[Vec<(RuleId, Span)>],
    from: RuleId,
    to: RuleId,
) -> bool {
    let RuleId(position) = to;
    let count = first_meta_identifiers.len();
    reachable(count, [from], |RuleId(rule), pending| {
        pending.extend(
            first_meta_identifiers[rule]
                .iter()
                .map(|&(target, _)| target),
        );
    })[position]
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        span::Span,
    };

    fn indirect(source: &str) -> Vec<(String, String, Span)> {
        findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::IndirectLeftRecursion {
                    meta_identifier,
                    through,
                } => Some((
                    meta_identifier.as_str().to_owned(),
                    through.as_str().to_owned(),
                    finding.span,
                )),
                _ => None,
            })
            .collect()
    }

    fn entry(
        rule: &str,
        through: &str,
        start: usize,
    ) -> (String, String, Span) {
        (
            rule.to_owned(),
            through.to_owned(),
            Span {
                start,
                end: start + 1,
            },
        )
    }

    #[test]
    fn finds_two_rules_that_start_with_each_other_once() {
        assert_eq!(
            indirect("s = a; a = b, 'x' | 'y'; b = a, 'z' | 'w';"),
            [entry("a", "b", 11)]
        );
    }

    #[test]
    fn finds_a_longer_chain_of_rules_once() {
        assert_eq!(
            indirect("s = a; a = b | 'x'; b = c | 'y'; c = a | 'z';"),
            [entry("a", "b", 11)]
        );
    }

    #[test]
    fn finds_a_chain_behind_parts_that_can_match_nothing() {
        assert_eq!(
            indirect("s = a; a = ['x'], b, 'y' | 'z'; b = a | 'w';"),
            [entry("a", "b", 18)]
        );
    }

    #[test]
    fn finds_each_cycle() {
        assert_eq!(
            indirect(
                "s = a | c; a = b, 'x' | 'y'; b = a, 'z' | 'w'; \
                 c = d, 'x' | 'y'; d = c, 'z' | 'w';"
            ),
            [entry("a", "b", 15), entry("c", "d", 51)]
        );
    }

    #[test]
    fn leaves_direct_left_recursion_to_its_own_check() {
        assert_eq!(indirect("a = a, 'x' | 'y';"), []);
    }

    #[test]
    fn accepts_rules_that_read_before_reaching_each_other() {
        assert_eq!(indirect("s = a; a = b | 'x'; b = 'y', a;"), []);
    }

    #[test]
    fn accepts_a_chain_that_does_not_lead_back() {
        assert_eq!(indirect("s = a; a = b | 'x'; b = c | 'y'; c = 'z';"), []);
    }
}
