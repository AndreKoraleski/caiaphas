//! An ineffective annotation changes nothing in its rule. A `(* greedy *)`
//! does not when a parser can already tell each way through the rule from the
//! next character, and the rule reaches no rule that tries every way. A
//! `(* first match *)` does not when no two alternatives of a choice in the
//! rule can match at the same place, since then the first that matches is also
//! the one that matches the most.

use crate::{
    analysis::{
        character_set::CharacterSet,
        exactness::{general_rules, visit_rule_conflicts},
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::{ExpressionKind, Grammar, Rule, RuleId},
};

/// Reports every `(* greedy *)` and `(* first match *)` that changes nothing
/// in its rule.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let general = general_rules(facts);
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        let mut ineffective = Vec::new();
        if rule.greedy {
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
                ineffective.push("greedy");
            }
        }
        if rule.first_match && !has_alternatives_that_meet(facts, rule) {
            ineffective.push("first match");
        }
        for annotation in ineffective {
            findings.push(Finding {
                kind: FindingKind::IneffectiveAnnotation {
                    meta_identifier: rule.meta_identifier.clone(),
                    annotation,
                },
                span: rule.span,
                labels: Vec::new(),
            });
        }
    }
}

/// Returns whether two alternatives of a choice in `rule` can match at the
/// same place, since one can match nothing or both can start alike.
fn has_alternatives_that_meet(facts: &Facts<'_>, rule: &Rule) -> bool {
    let mut meet = false;
    rule.expression.visit(&mut |expression| {
        let ExpressionKind::DefinitionsList(alternatives) = &expression.kind
        else {
            return;
        };
        let mut seen = CharacterSet::default();
        for alternative in alternatives {
            let first = facts.first(alternative);
            meet |= facts.is_nullable(alternative)
                || !seen.intersection(&first).is_empty();
            seen = seen.union(&first);
        }
    });
    meet
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

    #[test]
    fn finds_a_first_match_whose_alternatives_never_meet() {
        assert_eq!(
            ineffective("(* first match *) s = 'x' | 'y';"),
            [
                "`(* first match *)` changes nothing in `s`, since no two of \
                 its alternatives can match at the same place"
            ]
        );
        assert_eq!(
            ineffective("(* first match *) s = 'x' | 'xy';"),
            Vec::<String>::new()
        );
        assert_eq!(
            ineffective("(* first match *) s = 'x' | ;"),
            Vec::<String>::new()
        );
    }
}
