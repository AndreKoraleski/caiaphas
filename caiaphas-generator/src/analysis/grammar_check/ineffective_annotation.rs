//! An ineffective annotation changes nothing in its rule. A `(* greedy *)`
//! does not when a parser can already tell each way through the rule from the
//! next character, and the rule reaches no rule that tries every way.

use crate::{
    analysis::{
        exactness::{general_rules, visit_rule_conflicts},
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::{Grammar, RuleId},
};

/// Reports every `(* greedy *)` that changes nothing in its rule.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let general = general_rules(facts);
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        if !rule.greedy {
            continue;
        }
        let mut conflicts = false;
        visit_rule_conflicts(facts, RuleId(position), &mut |_| {
            conflicts = true;
        });
        let mut reaches_general = false;
        rule.expression
            .visit_meta_identifiers(&mut |RuleId(read), _| {
                reaches_general |= general[read];
            });
        if !conflicts && !reaches_general {
            findings.push(Finding {
                kind: FindingKind::IneffectiveAnnotation {
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
    use crate::analysis::{finding::FindingCode, tests::findings_with};

    fn ineffective(source: &str) -> Vec<String> {
        findings_with(source, FindingCode::IneffectiveAnnotation)
            .into_iter()
            .map(|finding| finding.kind.to_string())
            .collect()
    }

    #[test]
    fn finds_a_greedy_rule_whose_choices_the_next_character_makes() {
        assert_eq!(ineffective("(* greedy *) s = {'x'}, 'y';").len(), 1);
        assert_eq!(
            ineffective("(* greedy *) s = {'x'}, 'x';"),
            Vec::<String>::new()
        );
        assert_eq!(
            ineffective("(* greedy *) s = a, ';'; a = {'x'}, 'x';"),
            Vec::<String>::new()
        );
    }
}
