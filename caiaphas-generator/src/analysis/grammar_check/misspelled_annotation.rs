//! A misspelled annotation is a comment of one word right before a rule that
//! reads like an annotation written wrongly, such as `(* hidde *)`. To the
//! standard it is a comment like any other, so the rule is read as if it were
//! not there, but it is likely a typo that changes the rule's type.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::Grammar,
};

/// Reports every comment right before a rule that reads like a misspelled
/// annotation, at the rule, so an expectation of the rule can mark it.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for rule in &grammar.syntax_rules {
        for misspelling in &rule.misspellings {
            findings.push(Finding {
                kind: FindingKind::MisspelledAnnotation {
                    meant: misspelling.meant,
                },
                span: rule.span,
                labels: vec![Label {
                    span: misspelling.span,
                    message: "this comment",
                }],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingCode, tests::findings_with};

    fn misspelled(source: &str) -> Vec<String> {
        findings_with(source, FindingCode::MisspelledAnnotation)
            .into_iter()
            .map(|finding| finding.kind.to_string())
            .collect()
    }

    #[test]
    fn warns_of_a_word_written_almost_as_an_annotation() {
        assert_eq!(
            misspelled("(* HIDE *) a = 'x';"),
            ["this comment reads like `(* hide *)` written wrongly"]
        );
        assert_eq!(misspelled("(* hide the details *) a = 'x';"), [""; 0]);
    }

    #[test]
    fn lets_an_expectation_mark_one_as_intended() {
        assert_eq!(
            misspelled(
                "(* expect misspelled-annotation: a note *) (* Hidde *) \
                 a = 'x';"
            ),
            [""; 0]
        );
    }
}
