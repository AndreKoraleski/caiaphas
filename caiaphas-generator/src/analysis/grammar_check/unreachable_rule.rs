//! An unreachable rule cannot be reached from any start rule, so no parser
//! ever uses it, such as `a` and `b` in `s = 'x'; a = 'y', [b]; b = 'z', a;`.
//!
//! Grammar theory calls these unreachable symbols. Together with unproductive
//! rules they make up the useless symbols of a grammar. Since start rules are
//! the rules nothing references, an unreachable rule is always in a group of
//! rules that only reference each other.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::{Grammar, RuleId},
};

/// Reports every rule that no start rule can reach through references.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let mut reached = vec![false; grammar.syntax_rules.len()];
    let mut pending = grammar.start_syntax_rules.clone();
    while let Some(RuleId(position)) = pending.pop() {
        if reached[position] {
            continue;
        }
        reached[position] = true;
        grammar.syntax_rules[position]
            .definitions_list
            .meta_identifiers(&mut |rule_id, _| pending.push(rule_id));
    }
    for (rule, reached) in grammar.syntax_rules.iter().zip(reached) {
        if !reached {
            findings.push(Finding {
                kind: FindingKind::UnreachableRule {
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

    fn unreachable(source: &str) -> Vec<String> {
        findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::UnreachableRule { meta_identifier } => {
                    Some(meta_identifier.as_str().to_owned())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn finds_rules_that_only_reference_each_other() {
        assert_eq!(
            unreachable("s = 'x'; a = 'y', [b]; b = 'z', a;"),
            ["a", "b"]
        );
    }

    #[test]
    fn reaches_rules_through_every_kind_of_expression() {
        let source = "s = [a], {b}, (c | 3 * d), 'x' - e; a = 'a'; b = 'b'; \
                      c = 'c'; d = 'd'; e = 'e';";
        assert_eq!(unreachable(source), Vec::<String>::new());
    }

    #[test]
    fn reaches_rules_from_every_start_rule() {
        assert_eq!(
            unreachable("s = a; t = b; a = 'x'; b = 'y';"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn reaches_a_start_rule_that_references_itself() {
        assert_eq!(unreachable("s = 'x', [s];"), Vec::<String>::new());
    }
}
