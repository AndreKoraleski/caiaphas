//! An unproductive rule can never finish matching, such as `a = 'x', a;`,
//! which needs another `a` after every `x`.
//!
//! Grammar theory calls these non-generating or unproductive symbols. Every
//! rule that needs one in every alternative is unproductive too.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::Grammar,
};

/// Reports every rule that no input can finish matching.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for rule in &grammar.syntax_rules {
        if !facts.is_productive(&rule.expression) {
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
    fn finds_rules_that_only_need_each_other() {
        assert_eq!(
            unproductive("s = a; a = 'x', b; b = 'y', a;"),
            ["s", "a", "b"]
        );
    }

    #[test]
    fn accepts_a_rule_with_a_way_out() {
        assert_eq!(unproductive("a = 'x', a | 'y';"), Vec::<String>::new());
        assert_eq!(unproductive("a = 'x', [a];"), Vec::<String>::new());
        assert_eq!(unproductive("a = 'x', {a};"), Vec::<String>::new());
    }
}
