//! An empty rule can only match nothing, such as `a` in `s = a, 'x'; a = ;`,
//! so every reference to it reads no input.
//!
//! Grammar theory calls the rules that can match nothing nullable, and removes
//! the ones that can only match nothing when it normalizes a grammar. A rule
//! is empty when it can match nothing but cannot start with any character.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::Grammar,
};

/// Reports every rule that can match nothing and nothing else.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for rule in &grammar.syntax_rules {
        if facts.is_nullable(&rule.definitions_list)
            && facts.first(&rule.definitions_list).is_empty()
        {
            findings.push(Finding {
                kind: FindingKind::EmptyRule {
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

    fn empty(source: &str) -> Vec<String> {
        findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::EmptyRule { meta_identifier } => {
                    Some(meta_identifier.as_str().to_owned())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn finds_a_rule_that_is_an_empty_sequence() {
        assert_eq!(empty("s = a, 'x'; a = ;"), ["a"]);
    }

    #[test]
    fn finds_a_rule_that_only_references_empty_rules() {
        assert_eq!(empty("s = a, 'x'; a = [b], {b}; b = ;"), ["a", "b"]);
    }

    #[test]
    fn accepts_a_rule_that_can_read_something() {
        assert_eq!(empty("s = a, 'x'; a = ['y'];"), Vec::<String>::new());
        assert_eq!(empty("s = a, 'x'; a = 'y' | ;"), Vec::<String>::new());
    }

    #[test]
    fn accepts_a_rule_that_always_reads_something() {
        assert_eq!(empty("s = 'x';"), Vec::<String>::new());
    }
}
