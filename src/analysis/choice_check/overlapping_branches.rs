//! Overlapping branches are alternatives that share some of the characters
//! they can start with, so one character is not enough to pick between them.
//! Optional and repeated sequences are left to the follow conflict check.

use crate::{
    analysis::{
        choice_point::{ChoiceKind, ChoicePoint},
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
};

/// Reports every pair of branches that share some, but not all, of the
/// characters either can start with. When one has them all, the shadowed
/// branch check reports it instead.
pub(super) fn check(choice_point: &ChoicePoint, findings: &mut Vec<Finding>) {
    if choice_point.kind != ChoiceKind::DefinitionsList {
        return;
    }
    for (index, earlier) in choice_point.branches.iter().enumerate() {
        for later in &choice_point.branches[index + 1..] {
            let characters = earlier.first.intersection(&later.first);
            if characters.is_empty()
                || earlier.first.is_subset(&later.first)
                || later.first.is_subset(&earlier.first)
            {
                continue;
            }
            findings.push(Finding {
                kind: FindingKind::OverlappingBranches { characters },
                span: later.span,
                labels: vec![Label {
                    span: earlier.span,
                    message: "this branch can start the same way",
                }],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        diagnostic::Label,
        span::Span,
    };

    fn overlaps(source: &str) -> Vec<(String, Span, Vec<Label>)> {
        findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::OverlappingBranches { characters } => Some((
                    characters.to_string(),
                    finding.span,
                    finding.labels,
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn finds_alternatives_that_share_some_characters() {
        assert_eq!(
            overlaps("a = ? U+0061..U+0063 ? | ? U+0062..U+0064 ?;"),
            [(
                "'b'..='c'".to_owned(),
                Span { start: 25, end: 43 },
                vec![Label {
                    span: Span { start: 4, end: 22 },
                    message: "this branch can start the same way",
                }]
            )]
        );
    }

    #[test]
    fn leaves_optional_sequences_to_the_follow_conflict_check() {
        assert_eq!(
            overlaps("a = [? U+0061..U+0063 ?], ? U+0062..U+0064 ?;"),
            []
        );
    }

    #[test]
    fn accepts_branches_that_start_differently() {
        assert_eq!(overlaps("a = 'x' | 'y' | ['z'], 'w';"), []);
    }

    #[test]
    fn leaves_branches_that_contain_others_to_the_shadowed_check() {
        assert_eq!(overlaps("a = ? U+0061..U+007A ? | 'x';"), []);
    }
}
