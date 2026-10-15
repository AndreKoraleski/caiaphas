//! Names that differ only in case belong to different rules but read the same
//! aside from capital letters, such as `digit` and `Digit`. They are easy to
//! mistake for a typo or for one rule.
//!
//! Lints for confusable identifiers in programming languages warn about the
//! same thing.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::Grammar,
};

/// Reports every rule whose name differs only in case from an earlier rule's
/// name, against the first such rule.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let rules = &grammar.syntax_rules;
    for (index, rule) in rules.iter().enumerate() {
        let name = rule.meta_identifier.as_str();
        if let Some(earlier) = rules[..index].iter().find(|earlier| {
            earlier.meta_identifier.as_str().eq_ignore_ascii_case(name)
        }) {
            findings.push(Finding {
                kind: FindingKind::CaseOnlyNames {
                    meta_identifier: rule.meta_identifier.clone(),
                    other: earlier.meta_identifier.clone(),
                },
                span: rule.span,
                labels: vec![Label {
                    span: earlier.span,
                    message: "this rule has the same name in other case",
                }],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingKind, tests::findings};

    fn case_only(source: &str) -> Vec<(String, String)> {
        findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::CaseOnlyNames {
                    meta_identifier,
                    other,
                } => Some((
                    meta_identifier.as_str().to_owned(),
                    other.as_str().to_owned(),
                )),
                _ => None,
            })
            .collect()
    }

    fn pair(name: &str, other: &str) -> (String, String) {
        (name.to_owned(), other.to_owned())
    }

    #[test]
    fn finds_names_that_differ_only_in_case() {
        assert_eq!(
            case_only("s = digit, Digit; digit = '0'; Digit = '1';"),
            [pair("Digit", "digit")]
        );
    }

    #[test]
    fn reports_each_later_name_against_the_first() {
        assert_eq!(
            case_only("s = ab, AB, Ab; ab = 'a'; AB = 'b'; Ab = 'c';"),
            [pair("AB", "ab"), pair("Ab", "ab")]
        );
    }

    #[test]
    fn accepts_names_that_differ_in_letters() {
        assert_eq!(
            case_only("s = digit, digits; digit = '0'; digits = '1';"),
            []
        );
    }
}
