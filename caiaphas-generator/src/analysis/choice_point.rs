//! Choice points are the places where a parser has to pick one of several
//! branches by what comes next.

use crate::{
    analysis::{character_set::CharacterSet, facts::Facts},
    ir::{Expression, ExpressionKind, RuleId},
    span::Span,
};

/// A choice point and the branches it picks from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoicePoint {
    pub(crate) kind: ChoicePointKind,
    pub(crate) span: Span,
    pub(crate) branches: Vec<Branch>,
    /// Whether the input can end right after the choice point.
    pub(crate) end_of_input: bool,
    /// Whether its rule takes each way as it comes.
    pub(crate) greedy: bool,
}

/// What makes a [`ChoicePoint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoicePointKind {
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
    /// The characters a parser can see when the branch is the one to take.
    /// They are those it starts with, and what comes after the choice point
    /// when the branch can match nothing.
    pub(crate) lookahead: CharacterSet,
    pub(crate) nullable: bool,
    /// What [`Facts::single_characters`] returns for the branch.
    pub(crate) single_characters: Option<CharacterSet>,
}

impl ChoicePoint {
    /// Returns whether a parser can see the same character when either of two
    /// branches is the one to take, so it can try both at the same place.
    pub(crate) fn overlaps(&self) -> bool {
        self.branches.iter().enumerate().any(|(index, branch)| {
            self.branches[index + 1..].iter().any(|other| {
                !branch.lookahead.intersection(&other.lookahead).is_empty()
            })
        })
    }
}

/// Returns every choice point of the grammar `facts` answers about.
pub(crate) fn choice_points(facts: &Facts<'_>) -> Vec<ChoicePoint> {
    let mut choice_points = Vec::new();
    visit_choice_points(facts, &mut |_, choice_point| {
        choice_points.push(choice_point);
    });
    choice_points
}

/// Returns which rules a parser can try more than once at the same place, in
/// the grammar `facts` answers about. Only those have to remember their
/// matches.
///
/// They are the rules inside a choice point whose branches can start alike,
/// each rule that can match nothing and then be tried again right away, as `b`
/// can in `a = b, b;`, and every rule those reach.
pub(crate) fn revisited_rules(facts: &Facts<'_>) -> Vec<bool> {
    let mut starts = Vec::new();
    visit_choice_points(facts, &mut |expression, choice_point| {
        if choice_point.overlaps() {
            expression.visit_meta_identifiers(&mut |rule_id, _| {
                starts.push(rule_id);
            });
        }
    });
    for (position, rule) in facts.grammar().syntax_rules.iter().enumerate() {
        let rule_id = RuleId(position);
        if facts.is_nullable(&rule.expression) {
            let first = facts.first(&rule.expression);
            let after = &facts.follow(rule_id).characters;
            if first.is_empty() || !first.intersection(after).is_empty() {
                starts.push(rule_id);
            }
        }
    }
    facts.grammar().reached_from(starts)
}

/// Calls `visit` with every choice point of the grammar `facts` answers about,
/// and the expression that makes it.
fn visit_choice_points<'a>(
    facts: &Facts<'a>,
    visit: &mut impl FnMut(&'a Expression, ChoicePoint),
) {
    for (position, rule) in facts.grammar().syntax_rules.iter().enumerate() {
        facts.walk_rule(RuleId(position), &mut |expression, after| {
            let branch = |branch: &Expression| Branch {
                span: branch.span,
                lookahead: facts.first_before(branch, &after.characters),
                nullable: facts.is_nullable(branch),
                single_characters: facts.single_characters(branch),
            };
            let end = || Branch {
                span: Span {
                    start: expression.span.end,
                    end: expression.span.end,
                },
                lookahead: after.characters.clone(),
                nullable: true,
                single_characters: None,
            };
            let (kind, branches) = match &expression.kind {
                ExpressionKind::DefinitionsList(expressions) => (
                    ChoicePointKind::DefinitionsList,
                    expressions.iter().map(branch).collect(),
                ),
                ExpressionKind::OptionalSequence(expression) => (
                    ChoicePointKind::OptionalSequence,
                    vec![branch(expression), end()],
                ),
                ExpressionKind::RepeatedSequence(expression) => (
                    ChoicePointKind::RepeatedSequence,
                    vec![branch(expression), end()],
                ),
                _ => return,
            };
            let choice_point = ChoicePoint {
                kind,
                span: expression.span,
                branches,
                end_of_input: after.end_of_input,
                greedy: rule.greedy,
            };
            visit(expression, choice_point);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::build_valid;

    fn choice_points_of(source: &str) -> Vec<ChoicePoint> {
        let grammar = build_valid(source);
        choice_points(&Facts::new(&grammar))
    }

    fn firsts(choice_point: &ChoicePoint) -> Vec<String> {
        choice_point
            .branches
            .iter()
            .map(|branch| branch.lookahead.to_string())
            .collect()
    }

    #[test]
    fn finds_alternatives() {
        let points = choice_points_of("a = 'x' | 'y', 'z';");
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].kind, ChoicePointKind::DefinitionsList);
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
    }

    #[test]
    fn finds_taking_or_skipping_an_optional_sequence() {
        let points = choice_points_of("a = ['x'], 'y';");
        assert_eq!(points[0].kind, ChoicePointKind::OptionalSequence);
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
        assert!(points[0].branches[1].nullable);
        assert_eq!(points[0].branches[1].span, Span { start: 9, end: 9 });
    }

    #[test]
    fn finds_repeating_or_stopping_a_repeated_sequence() {
        let points = choice_points_of("a = {'x'}, 'y';");
        assert_eq!(points[0].kind, ChoicePointKind::RepeatedSequence);
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
    }

    fn revisited(source: &str) -> Vec<bool> {
        let grammar = build_valid(source);
        revisited_rules(&Facts::new(&grammar))
    }

    #[test]
    fn finds_rules_two_alternatives_reach_after_the_same_characters() {
        assert_eq!(
            revisited("s = a, 'x' | a, 'y'; a = b; b = 'z';"),
            [false, true, true]
        );
        assert_eq!(revisited("s = a | b; a = 'x'; b = 'y';"), [false; 3]);
    }

    #[test]
    fn finds_rules_a_sequence_and_what_follows_it_both_reach() {
        assert_eq!(revisited("s = [a, 'x'], a, 'y'; a = 'z';"), [false, true]);
        assert_eq!(revisited("s = [a, 'x'], 'y'; a = 'z';"), [false, false]);
        assert_eq!(revisited("s = {a, 'x'}, a, 'y'; a = 'z';"), [false, true]);
    }

    #[test]
    fn finds_rules_tried_again_where_they_matched_nothing() {
        assert_eq!(
            revisited("s = a, 'q'; a = b, b; b = c, c; c = 'x' | ;"),
            [false, false, true, true]
        );
    }

    #[test]
    fn counts_what_follows_a_branch_that_can_match_nothing() {
        let points = choice_points_of("a = ('x' | ), 'y';");
        assert_eq!(firsts(&points[0]), ["'x'", "'y'"]);
        assert!(points[0].branches[1].nullable);
    }
}
