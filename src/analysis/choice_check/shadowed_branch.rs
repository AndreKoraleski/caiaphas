//! A shadowed branch is an alternative that can only start with characters
//! another alternative of the same choice can start with too. Optional and
//! repeated sequences are left to the follow conflict check.

use crate::analysis::{
    choice_point::{ChoiceKind, ChoicePoint},
    finding::{Finding, FindingKind, Label},
};

/// Reports every branch whose characters another branch has all of, once,
/// against the first such branch. When two branches have the same characters,
/// only the later one is reported.
pub(super) fn check(choice_point: &ChoicePoint, findings: &mut Vec<Finding>) {
    if choice_point.kind != ChoiceKind::DefinitionsList {
        return;
    }
    let branches = &choice_point.branches;
    for (index, branch) in branches.iter().enumerate() {
        if branch.first.is_empty() {
            continue;
        }
        let shadowing =
            branches.iter().enumerate().find(|&(other, covering)| {
                other != index
                    && branch.first.is_subset(&covering.first)
                    && (other < index
                        || !covering.first.is_subset(&branch.first))
            });
        if let Some((_, covering)) = shadowing {
            findings.push(Finding {
                kind: FindingKind::ShadowedBranch,
                span: branch.span,
                labels: vec![Label {
                    span: covering.span,
                    message: "this branch can start with all of them",
                }],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        span::Span,
    };

    fn shadowed(source: &str) -> Vec<(Span, Span)> {
        findings(source)
            .into_iter()
            .filter(|finding| finding.kind == FindingKind::ShadowedBranch)
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    #[test]
    fn finds_a_later_branch_inside_an_earlier_one() {
        assert_eq!(
            shadowed("a = ? U+0061..U+007A ? | 'x';"),
            [(Span { start: 25, end: 28 }, Span { start: 4, end: 22 })]
        );
    }

    #[test]
    fn finds_an_earlier_branch_inside_a_later_one() {
        assert_eq!(
            shadowed("a = 'x' | ? U+0061..U+007A ?;"),
            [(Span { start: 4, end: 7 }, Span { start: 10, end: 28 })]
        );
    }

    #[test]
    fn leaves_optional_sequences_to_the_follow_conflict_check() {
        assert_eq!(shadowed("a = ['x'], 'x';"), []);
    }

    #[test]
    fn reports_each_branch_once() {
        assert_eq!(
            shadowed("a = ? U+0061..U+007A ? | 'x' | ? U+0078 ?;"),
            [
                (Span { start: 25, end: 28 }, Span { start: 4, end: 22 }),
                (Span { start: 31, end: 41 }, Span { start: 4, end: 22 }),
            ]
        );
    }

    #[test]
    fn accepts_branches_that_start_differently() {
        assert_eq!(shadowed("a = 'x' | 'y';"), []);
    }

    #[test]
    fn accepts_a_branch_that_can_start_with_nothing() {
        assert_eq!(shadowed("a = 'x' | ;"), []);
    }
}
