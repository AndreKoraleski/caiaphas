//! Choice points are the places where a parser has to pick a branch.

use crate::{
    analysis::{character_set::CharacterSet, facts::Facts},
    ir::{Expression, ExpressionKind},
    span::Span,
};

/// A choice point is a place where a parser has to pick one of its branches by
/// what comes next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoicePoint {
    pub(crate) kind: ChoiceKind,
    pub(crate) span: Span,
    pub(crate) branches: Vec<Branch>,
}

/// What makes a [`ChoicePoint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoiceKind {
    /// The branches are the alternatives.
    DefinitionsList,
    /// The branches are taking the sequence and skipping it.
    OptionalSequence,
    /// The branches are repeating the sequence again and stopping.
    RepeatedSequence,
}

/// A branch is one way a choice point can go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Branch {
    /// Skipping and stopping have an empty span at the end of the sequence.
    pub(crate) span: Span,
    /// The characters the branch can start with, counting what comes after the
    /// choice point when the branch can match nothing.
    pub(crate) first: CharacterSet,
    pub(crate) nullable: bool,
}

/// Returns every choice point of the grammar `facts` answers about.
pub(crate) fn choice_points(facts: &Facts<'_>) -> Vec<ChoicePoint> {
    let mut choice_points = Vec::new();
    facts.walk(&mut |expression, after| {
        let branch = |branch: &Expression| Branch {
            span: branch.span,
            first: facts.first_before(branch, &after.characters),
            nullable: facts.is_nullable(branch),
        };
        let end = || Branch {
            span: Span {
                start: expression.span.end,
                end: expression.span.end,
            },
            first: after.characters.clone(),
            nullable: true,
        };
        let (kind, branches) = match &expression.kind {
            ExpressionKind::DefinitionsList(expressions) => (
                ChoiceKind::DefinitionsList,
                expressions.iter().map(branch).collect(),
            ),
            ExpressionKind::OptionalSequence(expression) => (
                ChoiceKind::OptionalSequence,
                vec![branch(expression), end()],
            ),
            ExpressionKind::RepeatedSequence(expression) => (
                ChoiceKind::RepeatedSequence,
                vec![branch(expression), end()],
            ),
            _ => return,
        };
        choice_points.push(ChoicePoint {
            kind,
            span: expression.span,
            branches,
        });
    });
    choice_points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dictionary::build, parser::parse};

    fn choice_points_of(source: &str) -> Vec<ChoicePoint> {
        let grammar = build(&parse(source).0).0;
        choice_points(&Facts::new(&grammar))
    }

    fn firsts(choice_point: &ChoicePoint) -> Vec<String> {
        choice_point
            .branches
            .iter()
            .map(|branch| branch.first.to_string())
            .collect()
    }

    #[test]
    fn finds_alternatives() {
        let points = choice_points_of("a = 'x' | 'y', 'z';");
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].kind, ChoiceKind::DefinitionsList);
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
    }

    #[test]
    fn finds_taking_or_skipping_an_optional_sequence() {
        let points = choice_points_of("a = ['x'], 'y';");
        assert_eq!(points[0].kind, ChoiceKind::OptionalSequence);
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
        assert!(points[0].branches[1].nullable);
        assert_eq!(points[0].branches[1].span, Span { start: 9, end: 9 });
    }

    #[test]
    fn finds_repeating_or_stopping_a_repeated_sequence() {
        let points = choice_points_of("a = {'x'}, 'y';");
        assert_eq!(points[0].kind, ChoiceKind::RepeatedSequence);
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
    }

    #[test]
    fn counts_what_follows_a_branch_that_can_match_nothing() {
        let points = choice_points_of("a = ('x' | ), 'y';");
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
        assert!(points[0].branches[1].nullable);
    }
}
