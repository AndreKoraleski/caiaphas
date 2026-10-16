//! Expectations mark findings in a rule as intended, so the analysis leaves
//! them out.

use crate::{
    analysis::finding::{Finding, FindingCode, FindingKind},
    diagnostic::Label,
    ir::Grammar,
};

/// Removes every finding a rule expects, and reports every expectation that
/// names no kind of finding, names one that cannot be intended, or finds
/// nothing in its rule.
pub(super) fn apply(grammar: &Grammar, findings: &mut Vec<Finding>) {
    let mut reported = Vec::new();
    for rule in &grammar.syntax_rules {
        for expectation in &rule.expectations {
            let Some(code) = FindingCode::from_name(&expectation.code) else {
                reported.push(Finding {
                    kind: FindingKind::UnknownExpectation {
                        code: expectation.code.clone(),
                    },
                    span: expectation.span,
                    labels: Vec::new(),
                });
                continue;
            };
            if !code.is_expectable() {
                reported.push(Finding {
                    kind: FindingKind::UnexpectableFinding { code },
                    span: expectation.span,
                    labels: Vec::new(),
                });
                continue;
            }
            let before = findings.len();
            findings.retain(|finding| {
                finding.kind.code() != code
                    || finding.span.start < rule.span.start
                    || finding.span.end > rule.span.end
            });
            if findings.len() == before {
                reported.push(Finding {
                    kind: FindingKind::UnfulfilledExpectation { code },
                    span: expectation.span,
                    labels: vec![Label {
                        span: rule.span,
                        message: "in this rule",
                    }],
                });
            }
        }
    }
    findings.extend(reported);
}

#[cfg(test)]
mod tests {
    use crate::analysis::{
        finding::{FindingCode, FindingKind},
        tests::{findings, greedy_findings},
    };

    /// Returns the codes found in the grammar `source` with every rule greedy,
    /// so a parser misses what a greedy one does.
    fn codes(source: &str) -> Vec<&'static str> {
        greedy_findings(source)
            .iter()
            .map(|finding| finding.kind.code().name())
            .collect()
    }

    #[test]
    fn leaves_out_an_expected_finding() {
        assert_eq!(codes("a = ['xy'], 'x';"), ["optional-follow-conflict"]);
        assert_eq!(
            codes(
                "(* expect optional-follow-conflict: taken whenever it can \
                 be *) a = ['xy'], 'x';"
            ),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn leaves_out_only_the_expected_kind() {
        assert_eq!(
            codes(
                "(* expect optional-follow-conflict: wanted *) \
                 a = ['xy'], 'x' | 'x';"
            ),
            ["tied-branches"]
        );
    }

    #[test]
    fn leaves_out_every_finding_of_the_kind_in_the_rule() {
        assert_eq!(
            codes(
                "(* expect optional-follow-conflict: wanted *) \
                 a = ['xy'], 'x', ['yz'], 'y';"
            ),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn leaves_findings_in_other_rules() {
        assert_eq!(
            codes(
                "(* expect optional-follow-conflict: wanted *) \
                 a = ['xy'], 'x', b; b = ['yz'], 'y';"
            ),
            ["optional-follow-conflict"]
        );
    }

    #[test]
    fn leaves_findings_in_rules_before_it() {
        assert_eq!(
            codes(
                "s = b; b = ['yz'], 'y', a; \
                 (* expect optional-follow-conflict: wanted *) \
                 a = ['xy'], 'x';"
            ),
            ["optional-follow-conflict"]
        );
    }

    #[test]
    fn leaves_out_a_finding_about_the_whole_rule() {
        assert_eq!(
            codes(
                "s = a, 'x'; \
                 (* expect empty-rule: a placeholder for now *) a = ;"
            ),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn accepts_several_expectations_for_one_rule() {
        assert_eq!(
            codes(
                "(* expect optional-follow-conflict: wanted *) \
                 (* expect tied-branches: also wanted *) \
                 a = ['xy'], 'x' | 'x';"
            ),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn does_not_hide_a_repetition_that_takes_everything_after_it() {
        assert_eq!(
            codes(
                "(* expect repeated-follow-conflict: wanted *) \
                 a = {'x'}, 'x';"
            ),
            ["unfulfilled-expectation", "dead-end-repeated-sequence"]
        );
    }

    #[test]
    fn reports_an_unknown_code() {
        assert_eq!(
            codes("(* expect no-such-finding: none *) a = 'x';"),
            ["unknown-expectation"]
        );
    }

    #[test]
    fn reports_an_expected_error_and_keeps_the_error() {
        assert_eq!(
            codes("(* expect left-recursion: wanted *) a = a, 'x' | 'y';"),
            ["unexpectable-finding", "left-recursion"]
        );
    }

    #[test]
    fn reports_an_expectation_with_nothing_to_expect() {
        let found = findings("(* expect empty-rule: kept *) a = 'x';");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].kind,
            FindingKind::UnfulfilledExpectation {
                code: FindingCode::EmptyRule
            }
        );
        assert_eq!(found[0].labels[0].message, "in this rule");
    }

    #[test]
    fn reports_a_second_expectation_of_the_same_kind() {
        assert_eq!(
            codes(
                "(* expect optional-follow-conflict: wanted *) \
                 (* expect optional-follow-conflict: again *) \
                 a = ['xy'], 'x';"
            ),
            ["unfulfilled-expectation"]
        );
    }

    #[test]
    fn cannot_expect_an_unfulfilled_expectation() {
        let source = "(* expect unfulfilled-expectation: none *) a = 'x';";
        assert_eq!(codes(source), ["unexpectable-finding"]);
        assert_eq!(
            findings(source)[0].kind.to_string(),
            "`unfulfilled-expectation` is about expectations, so it cannot be \
             expected"
        );
    }
}
