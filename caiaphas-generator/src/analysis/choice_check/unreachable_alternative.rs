//! An unreachable alternative comes after an alternative that can match
//! nothing in a rule that takes the first match, such as `'x'` in
//! `(* first match *) a = ['y'] | 'x';`. The earlier one always matches, if
//! only nothing, so a parser never tries the later one.

use crate::{
    analysis::{
        choice_point::{ChoicePoint, ChoicePointKind},
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
};

/// Reports every alternative of a rule that takes the first match written
/// after one that can match nothing.
pub(super) fn check(choice_point: &ChoicePoint, findings: &mut Vec<Finding>) {
    if choice_point.kind != ChoicePointKind::FirstMatchDefinitionsList {
        return;
    }
    let branches = &choice_point.branches;
    let Some(index) = branches.iter().position(|branch| branch.nullable)
    else {
        return;
    };
    for branch in &branches[index + 1..] {
        findings.push(Finding {
            kind: FindingKind::UnreachableAlternative,
            span: branch.span,
            labels: vec![Label {
                span: branches[index].span,
                message: "this alternative can match nothing",
            }],
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn unreachable(source: &str) -> Vec<(Span, Span)> {
        findings_with(source, FindingCode::UnreachableAlternative)
            .into_iter()
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    #[test]
    fn finds_every_alternative_after_one_that_can_match_nothing() {
        assert_eq!(
            unreachable("(* first match *) a = 'z' | ['y'] | 'x' | 'w';"),
            [
                (Span { start: 36, end: 39 }, Span { start: 28, end: 33 }),
                (Span { start: 42, end: 45 }, Span { start: 28, end: 33 }),
            ]
        );
    }

    #[test]
    fn accepts_one_that_can_match_nothing_written_last() {
        assert_eq!(unreachable("(* first match *) a = 'x' | ['y'];"), []);
    }

    #[test]
    fn leaves_rules_that_take_the_longest_match_alone() {
        assert_eq!(unreachable("a = ['y'] | 'x';"), []);
    }
}
