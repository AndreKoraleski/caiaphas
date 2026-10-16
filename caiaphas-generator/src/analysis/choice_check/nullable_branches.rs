//! Nullable branches can match nothing, and a choice with more than one of
//! them has more than one way to match nothing.

use crate::{
    analysis::{
        choice_point::{ChoicePoint, ChoicePointKind},
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
};

/// Reports every choice point with more than one branch that can match
/// nothing. For a repeated sequence that means its body can match nothing, so
/// it could repeat forever.
///
/// Only the alternatives of a definitions list get labels, since the finding
/// already points at the only body of an optional or repeated sequence.
pub(super) fn check(choice_point: &ChoicePoint, findings: &mut Vec<Finding>) {
    let nullable: Vec<Label> = choice_point
        .branches
        .iter()
        .filter(|branch| branch.nullable)
        .map(|branch| Label {
            span: branch.span,
            message: "this branch can match nothing",
        })
        .collect();
    if nullable.len() < 2 {
        return;
    }
    let (kind, labels) = match choice_point.kind {
        ChoicePointKind::DefinitionsList => {
            (FindingKind::AmbiguousNullableBranches, nullable)
        }
        ChoicePointKind::OptionalSequence => {
            (FindingKind::AmbiguousNullableBranches, Vec::new())
        }
        ChoicePointKind::RepeatedSequence => {
            (FindingKind::NullableRepeatedSequence, Vec::new())
        }
    };
    findings.push(Finding {
        kind,
        span: choice_point.span,
        labels,
    });
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        span::Span,
    };

    fn empty(source: &str) -> Vec<(FindingKind, Span, usize)> {
        findings(source)
            .into_iter()
            .filter(|finding| {
                matches!(
                    finding.kind,
                    FindingKind::AmbiguousNullableBranches
                        | FindingKind::NullableRepeatedSequence
                )
            })
            .map(|finding| (finding.kind, finding.span, finding.labels.len()))
            .collect()
    }

    #[test]
    fn finds_alternatives_that_can_both_match_nothing() {
        assert_eq!(
            empty("a = ['x'] | ['y'];"),
            [(
                FindingKind::AmbiguousNullableBranches,
                Span { start: 4, end: 17 },
                2
            )]
        );
    }

    #[test]
    fn finds_an_optional_sequence_that_can_match_nothing() {
        assert_eq!(
            empty("a = [['x']], 'y';"),
            [(
                FindingKind::AmbiguousNullableBranches,
                Span { start: 4, end: 11 },
                0
            )]
        );
    }

    #[test]
    fn finds_a_repeated_sequence_that_can_match_nothing() {
        assert_eq!(
            empty("a = {['x']};"),
            [(
                FindingKind::NullableRepeatedSequence,
                Span { start: 4, end: 11 },
                0
            )]
        );
    }

    #[test]
    fn accepts_one_way_to_match_nothing() {
        assert_eq!(empty("a = ['x'] | 'y';"), []);
        assert_eq!(empty("a = {'x'};"), []);
    }
}
