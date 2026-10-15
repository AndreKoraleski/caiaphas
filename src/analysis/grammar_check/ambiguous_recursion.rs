//! Ambiguous recursion lets a rule group the same input in more than one way,
//! such as `e = e, '+', e | 'n';` reading `n+n+n`.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::{ExpressionKind, Grammar, RuleId},
};

/// Reports every alternative of a rule that both starts and ends with the rule
/// itself.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        let alternatives = match &rule.definitions_list.kind {
            ExpressionKind::DefinitionsList(expressions) => {
                expressions.iter().collect()
            }
            _ => vec![&rule.definitions_list],
        };
        let is_this_rule = |(target, _): &_| *target == RuleId(position);
        for alternative in alternatives {
            if facts
                .first_meta_identifiers(alternative)
                .iter()
                .any(is_this_rule)
                && facts
                    .last_meta_identifiers(alternative)
                    .iter()
                    .any(is_this_rule)
            {
                findings.push(Finding {
                    kind: FindingKind::AmbiguousRecursion {
                        meta_identifier: rule.meta_identifier.clone(),
                    },
                    span: alternative.span,
                    labels: Vec::new(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        span::Span,
    };

    fn ambiguous_recursions(source: &str) -> Vec<Span> {
        findings(source)
            .into_iter()
            .filter(|finding| {
                matches!(finding.kind, FindingKind::AmbiguousRecursion { .. })
            })
            .map(|finding| finding.span)
            .collect()
    }

    #[test]
    fn finds_an_alternative_recursive_at_both_ends() {
        assert_eq!(
            ambiguous_recursions("e = e, '+', e | 'n';"),
            [Span { start: 4, end: 13 }]
        );
    }

    #[test]
    fn finds_a_rule_recursive_at_both_ends() {
        assert_eq!(
            ambiguous_recursions("e = e, '+', [e];"),
            [Span { start: 4, end: 15 }]
        );
    }

    #[test]
    fn accepts_recursion_at_one_end() {
        assert_eq!(ambiguous_recursions("e = 'n', '+', e | 'n';"), []);
        assert_eq!(ambiguous_recursions("e = e, '+', 'n' | 'n';"), []);
    }
}
