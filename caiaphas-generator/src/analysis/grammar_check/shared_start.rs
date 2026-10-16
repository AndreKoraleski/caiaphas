//! A shared start is the beginning two alternatives of one choice are both
//! written with, such as `a` in `s = a, 'x' | a, 'y';`. A parser reads it once
//! for each alternative, and has to remember what the rules in it matched to
//! read them only once.
//!
//! Written once before a choice of what follows it, as in
//! `s = a, ('x' | 'y');`, the start matches the same and is read once. That
//! makes this a suggestion, which never stops the build.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
        grammar_check::longer_branch_conflict::rest_after,
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar},
};

/// Suggests writing once the start every alternative shares with an earlier
/// alternative of the same definitions list.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for rule in &grammar.syntax_rules {
        rule.expression.visit(&mut |expression| {
            let ExpressionKind::DefinitionsList(alternatives) =
                &expression.kind
            else {
                return;
            };
            for (index, later) in alternatives.iter().enumerate() {
                let earlier = alternatives[..index].iter().find(|earlier| {
                    shares_start(earlier, later)
                        // A first match never takes an alternative that starts
                        // with all of an earlier one.
                        && !(rule.first_match
                            && rest_after(earlier, later).is_some())
                });
                if let Some(earlier) = earlier {
                    findings.push(Finding {
                        kind: FindingKind::SharedStart,
                        span: later.span,
                        labels: vec![Label {
                            span: earlier.span,
                            message: "this alternative starts the same way",
                        }],
                    });
                }
            }
        });
    }
}

/// Returns whether `earlier` and `later` start with the same expression, and
/// are not written the same way, which the duplicate alternative check
/// reports.
fn shares_start(earlier: &Expression, later: &Expression) -> bool {
    let (Some(start), Some(other)) =
        (earlier.steps().first(), later.steps().first())
    else {
        return false;
    };
    start.is_written_like(other) && !earlier.is_written_like(later)
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn suggested(source: &str) -> Vec<(Span, Span)> {
        findings_with(source, FindingCode::SharedStart)
            .into_iter()
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    #[test]
    fn suggests_writing_a_shared_start_once() {
        assert_eq!(
            suggested("s = a, 'x' | a, 'y'; a = 'z';"),
            [(Span { start: 13, end: 19 }, Span { start: 4, end: 10 })]
        );
        assert_eq!(
            suggested("s = a | a, 'y'; a = 'z';"),
            [(Span { start: 8, end: 14 }, Span { start: 4, end: 5 })]
        );
    }

    #[test]
    fn suggests_it_where_a_rule_takes_the_first_match() {
        assert_eq!(
            suggested("(* first match *) s = a, 'x' | a, 'y'; a = 'z';").len(),
            1
        );
    }

    #[test]
    fn leaves_what_other_checks_report_alone() {
        assert_eq!(suggested("s = a | a; a = 'z';"), []);
        assert_eq!(
            suggested("(* first match *) s = a | a, 'y'; a = 'z';"),
            []
        );
    }

    #[test]
    fn accepts_alternatives_that_start_differently() {
        assert_eq!(suggested("s = a, 'x' | 'y', a; a = 'z';"), []);
    }
}
