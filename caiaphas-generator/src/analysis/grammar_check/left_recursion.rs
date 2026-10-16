//! Left recursion makes a parser call a rule from inside itself before reading
//! anything, so it never ends.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::{Grammar, RuleId},
};

/// Reports every rule that can reach itself before reading a character,
/// directly or behind parts that can match nothing.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        for (target, span) in facts.first_meta_identifiers(&rule.expression) {
            if target == RuleId(position) {
                findings.push(Finding {
                    kind: FindingKind::LeftRecursion {
                        meta_identifier: rule.meta_identifier.clone(),
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

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{
            finding::{FindingCode, FindingKind},
            tests::{findings, findings_with},
        },
        diagnostic::Label,
        span::Span,
    };

    fn left_recursions(source: &str) -> Vec<Span> {
        findings_with(source, FindingCode::LeftRecursion)
            .into_iter()
            .map(|finding| finding.span)
            .collect()
    }

    #[test]
    fn finds_a_rule_that_starts_with_itself() {
        assert_eq!(
            left_recursions("a = a, 'x' | 'y';"),
            [Span { start: 4, end: 5 }]
        );
    }

    #[test]
    fn finds_a_rule_that_starts_with_itself_behind_nullable_parts() {
        assert_eq!(
            left_recursions("a = ['x'], {'y'}, a, 'z' | 'w';"),
            [Span { start: 18, end: 19 }]
        );
    }

    #[test]
    fn points_at_the_rule() {
        let finding = findings("b = 'x'; a = a, 'x' | 'y';")
            .into_iter()
            .find(|finding| {
                matches!(finding.kind, FindingKind::LeftRecursion { .. })
            })
            .expect("a is left recursive");
        assert_eq!(
            finding.labels,
            [Label {
                span: Span { start: 9, end: 26 },
                message: "in this rule",
            }]
        );
    }

    #[test]
    fn accepts_a_rule_that_reads_before_itself() {
        assert_eq!(left_recursions("a = 'x', a | 'y';"), []);
        assert_eq!(left_recursions("a = 'x', [a];"), []);
    }
}
