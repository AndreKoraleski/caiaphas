//! A follow conflict is an optional or repeated sequence that can start with a
//! character that can also come right after it, such as the dangling else in
//! `s = 'if', c, s, ['else', s] | 'x';`, where an `else` after the inner `s`
//! could belong to either `if`. LL(1) parsing calls this a FIRST/FOLLOW
//! conflict.
//!
//! Only a greedy rule reports it, since a greedy rule takes the sequence
//! whenever it matches and every other rule tries both ways. When the sequence
//! only ever matches one character, it can take every character that can come
//! after it, and the input cannot end there, the finding is a required
//! optional sequence or a dead-end repeated sequence instead, as in
//! `s = {'x'}, 'x';`.

use crate::analysis::{
    character_set::CharacterSet,
    choice_point::{ChoicePoint, ChoicePointKind},
    finding::{Finding, FindingKind},
};

/// Reports every optional or repeated sequence of a greedy rule that can start
/// with what can come right after it. A sequence that can match nothing is
/// left to the nullable branches check.
pub(super) fn check(choice_point: &ChoicePoint, findings: &mut Vec<Finding>) {
    if !choice_point.greedy {
        return;
    }
    // What the sequence is reported as when it can start like what follows it,
    // and when it takes everything that follows it.
    let (conflict, takes_all): (fn(CharacterSet) -> FindingKind, _) =
        match choice_point.kind {
            ChoicePointKind::OptionalSequence => (
                |characters| FindingKind::OptionalFollowConflict {
                    characters,
                },
                FindingKind::RequiredOptionalSequence,
            ),
            ChoicePointKind::RepeatedSequence => (
                |characters| FindingKind::RepeatedFollowConflict {
                    characters,
                },
                FindingKind::DeadEndRepeatedSequence,
            ),
            ChoicePointKind::DefinitionsList
            | ChoicePointKind::FirstMatchDefinitionsList => return,
        };
    let [sequence, end] = choice_point.branches.as_slice() else {
        return;
    };
    if sequence.nullable {
        return;
    }
    let characters = sequence.lookahead.intersection(&end.lookahead);
    if characters.is_empty() {
        return;
    }
    let takes_everything_after = !choice_point.end_of_input
        && sequence
            .single_characters
            .as_ref()
            .is_some_and(|taken| end.lookahead.is_subset(taken));
    let kind = if takes_everything_after {
        takes_all
    } else {
        conflict(characters)
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
        analysis::{finding::FindingKind, tests::greedy_findings},
        span::Span,
    };

    fn conflicts(source: &str) -> Vec<(String, Span)> {
        greedy_findings(source)
            .into_iter()
            .filter_map(|finding| match finding.kind {
                FindingKind::OptionalFollowConflict { characters } => {
                    Some((format!("optional {characters}"), finding.span))
                }
                FindingKind::RepeatedFollowConflict { characters } => {
                    Some((format!("repeated {characters}"), finding.span))
                }
                FindingKind::RequiredOptionalSequence => {
                    Some(("required".to_owned(), finding.span))
                }
                FindingKind::DeadEndRepeatedSequence => {
                    Some(("dead end".to_owned(), finding.span))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn finds_an_optional_sequence_that_starts_like_what_follows_it() {
        assert_eq!(
            conflicts("a = ['xy'], 'x';"),
            [("optional 'x'".to_owned(), Span { start: 4, end: 10 })]
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
            conflicts("s = a, 'x'; a = 'y', ['xy'];"),
            [("optional 'x'".to_owned(), Span { start: 21, end: 27 })]
        );
    }

    #[test]
    fn accepts_a_sequence_that_starts_differently_from_what_follows() {
        assert_eq!(conflicts("a = ['x'], {'y'}, 'z';"), []);
    }

    #[test]
    fn leaves_a_sequence_that_can_match_nothing_to_the_empty_branches_check() {
        assert_eq!(conflicts("a = [{'x'}], 'y';"), []);
    }

    #[test]
    fn finds_an_optional_sequence_that_takes_everything_after_it() {
        assert_eq!(
            conflicts("a = ['x'], 'x';"),
            [("required".to_owned(), Span { start: 4, end: 9 })]
        );
    }

    #[test]
    fn finds_a_repeated_sequence_that_takes_everything_after_it() {
        assert_eq!(
            conflicts("a = {? U+0061..U+007A ?}, 'x';"),
            [("dead end".to_owned(), Span { start: 4, end: 24 })]
        );
    }

    #[test]
    fn leaves_a_way_past_a_sequence_where_the_input_can_end() {
        assert_eq!(
            conflicts("a = {'x'}, ['x'];"),
            [("repeated 'x'".to_owned(), Span { start: 4, end: 9 })]
        );
    }

    #[test]
    fn leaves_a_way_past_a_sequence_of_more_than_one_character() {
        assert_eq!(
            conflicts("a = {'xy'}, 'x';"),
            [("repeated 'x'".to_owned(), Span { start: 4, end: 10 })]
        );
    }
}
