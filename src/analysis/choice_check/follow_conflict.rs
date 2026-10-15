//! A follow conflict is an optional or repeated sequence that can start with a
//! character that can also come right after it, such as the dangling else in
//! `s = 'if', c, s, ['else', s] | 'x';`, where an `else` after the inner `s`
//! could belong to either `if`.
//!
//! LL(1) parsing calls this a FIRST/FOLLOW conflict. A parser that sees one of
//! those characters cannot tell whether to take the sequence, or whether to
//! repeat it again, so it has to pick one, usually the sequence.

use crate::analysis::{
    choice_point::{ChoiceKind, ChoicePoint},
    finding::{Finding, FindingKind},
};

/// Reports every optional or repeated sequence that can start with what can
/// come right after it, unless it can match nothing.
pub(super) fn check(choice_point: &ChoicePoint, findings: &mut Vec<Finding>) {
    let [sequence, end] = choice_point.branches.as_slice() else {
        return;
    };
    if sequence.nullable {
        return;
    }
    let characters = sequence.first.intersection(&end.first);
    if characters.is_empty() {
        return;
    }
    let kind = match choice_point.kind {
        ChoiceKind::OptionalSequence => {
            FindingKind::OptionalFollowConflict { characters }
        }
        ChoiceKind::RepeatedSequence => {
            FindingKind::RepeatedFollowConflict { characters }
        }
        ChoiceKind::DefinitionsList => return,
    };
    findings.push(Finding {
        kind,
        span: choice_point.span,
        labels: Vec::new(),
    });
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        span::Span,
    };

    fn conflicts(source: &str) -> Vec<(String, Span)> {
        findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::OptionalFollowConflict { characters } => {
                    Some((format!("optional {characters}"), finding.span))
                }
                FindingKind::RepeatedFollowConflict { characters } => {
                    Some((format!("repeated {characters}"), finding.span))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn finds_an_optional_sequence_that_starts_like_what_follows_it() {
        assert_eq!(
            conflicts("a = ['x'], 'x';"),
            [("optional 'x'".to_owned(), Span { start: 4, end: 9 })]
        );
    }

    #[test]
    fn finds_a_repeated_sequence_that_starts_like_what_follows_it() {
        assert_eq!(
            conflicts("a = {'x'}, ? U+0077..U+0078 ?;"),
            [("repeated 'x'".to_owned(), Span { start: 4, end: 9 })]
        );
    }

    #[test]
    fn finds_what_follows_through_other_rules() {
        assert_eq!(
            conflicts("s = a, 'x'; a = 'y', ['x'];"),
            [("optional 'x'".to_owned(), Span { start: 21, end: 26 })]
        );
    }

    #[test]
    fn accepts_a_sequence_that_starts_differently_from_what_follows() {
        assert_eq!(conflicts("a = ['x'], {'y'}, 'z';"), []);
    }

    #[test]
    fn skips_a_sequence_that_can_match_nothing() {
        assert_eq!(conflicts("a = [{'x'}], 'y';"), []);
    }
}
