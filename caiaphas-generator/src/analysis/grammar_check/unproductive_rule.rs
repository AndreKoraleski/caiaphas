//! An unproductive rule can never finish matching, such as `a = 'x', a;`,
//! which needs another `a` after every `x`.
//!
//! Grammar theory calls these non-generating or unproductive symbols. Every
//! rule that needs one in every alternative is unproductive too, but only the
//! first rule of each cycle of them is reported, since fixing it fixes the
//! rest.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::{Grammar, RuleId, reachable},
};

/// Reports the first rule of each cycle of rules that no input can finish
/// matching.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let rules = &grammar.syntax_rules;
    let unproductive: Vec<bool> = rules
        .iter()
        .map(|rule| !facts.is_productive(&rule.expression))
        .collect();
    // The unproductive rules that each rule references.
    let needs: Vec<Vec<RuleId>> = rules
        .iter()
        .map(|rule| {
            let mut needed = Vec::new();
            rule.expression.visit_meta_identifiers(&mut |target, _| {
                let RuleId(position) = target;
                if unproductive[position] {
                    needed.push(target);
                }
            });
            needed
        })
        .collect();
    let reaches = |from: usize, to: usize| {
        reachable(rules.len(), needs[from].iter().copied(), |rule, pending| {
            let RuleId(position) = rule;
            pending.extend(needs[position].iter().copied());
        })[to]
    };
    for (position, rule) in rules.iter().enumerate() {
        let first_of_cycle = unproductive[position]
            && reaches(position, position)
            && !(0..position).any(|earlier| {
                reaches(earlier, position) && reaches(position, earlier)
            });
        if first_of_cycle {
            findings.push(Finding {
                kind: FindingKind::UnproductiveRule {
                    meta_identifier: rule.meta_identifier.clone(),
                },
                span: rule.span,
                labels: Vec::new(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingKind, tests::findings};

    fn unproductive(source: &str) -> Vec<String> {
        findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::UnproductiveRule { meta_identifier } => {
                    Some(meta_identifier.as_str().to_owned())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn finds_a_rule_that_always_needs_itself_again() {
        assert_eq!(unproductive("a = 'x', a;"), ["a"]);
    }

    #[test]
    fn finds_rules_that_only_need_each_other_once() {
        assert_eq!(unproductive("s = a; a = 'x', b; b = 'y', a;"), ["a"]);
    }

    #[test]
    fn accepts_a_rule_with_a_way_out() {
        assert_eq!(unproductive("a = 'x', a | 'y';"), Vec::<String>::new());
        assert_eq!(unproductive("a = 'x', [a];"), Vec::<String>::new());
        assert_eq!(unproductive("a = 'x', {a};"), Vec::<String>::new());
    }
}
