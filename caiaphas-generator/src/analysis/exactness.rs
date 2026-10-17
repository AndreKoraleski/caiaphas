//! Exactness tells where a parser that takes one way through each choice as it
//! comes to it can miss a match.
//!
//! Such a parser takes the alternative that matches the most, and takes or
//! repeats a sequence whenever it can. It misses a match only where a way it
//! leaves out ends sooner and the rest goes on from there.

use crate::{
    analysis::{
        character_set::CharacterSet,
        facts::{Facts, Follow},
    },
    ir::{Expression, ExpressionKind, RuleId, reachable},
    span::Span,
};

/// A choice that a parser taking one way as it comes can get wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conflict {
    pub(crate) rule_id: RuleId,
    /// The optional or repeated sequence, or the alternative that goes on from
    /// where another ends.
    pub(crate) span: Span,
    /// The alternative that ends where the other goes on.
    pub(crate) shorter: Option<Span>,
    /// The characters the way that goes on can read next.
    pub(crate) characters: CharacterSet,
}

/// Calls `visit` with each conflict of the rules that are not greedy in the
/// grammar `facts` answers about.
pub(crate) fn visit_conflicts(
    facts: &Facts<'_>,
    visit: &mut impl FnMut(Conflict),
) {
    for (position, rule) in facts.grammar().syntax_rules.iter().enumerate() {
        if !rule.greedy {
            visit_rule_conflicts(facts, RuleId(position), visit);
        }
    }
}

/// Calls `visit` with each conflict of the rule `rule_id`, greedy or not.
pub(crate) fn visit_rule_conflicts(
    facts: &Facts<'_>,
    rule_id: RuleId,
    visit: &mut impl FnMut(Conflict),
) {
    facts.walk_rule(rule_id, &mut |expression, after| {
        if let Some(conflict) = conflict(facts, rule_id, expression, after) {
            visit(conflict);
        }
    });
}

/// Returns which rules of the grammar `facts` answers about try every way
/// through them: those with a conflict, and every rule that reaches one other
/// than through a greedy rule, since only what comes after a rule tells which
/// way the input needs.
pub(crate) fn general_rules(facts: &Facts<'_>) -> Vec<bool> {
    let rules = &facts.grammar().syntax_rules;
    let mut inexact = Vec::new();
    visit_conflicts(facts, &mut |conflict| inexact.push(conflict.rule_id));
    let mut callers = vec![Vec::new(); rules.len()];
    for (position, rule) in rules.iter().enumerate() {
        if !rule.greedy {
            rule.expression
                .visit_meta_identifiers(&mut |RuleId(read), _| {
                    callers[read].push(RuleId(position));
                });
        }
    }
    reachable(rules.len(), inexact, |RuleId(position), pending| {
        pending.extend(&callers[position]);
    })
}

/// Returns how a parser that takes one way through `expression` as it comes to
/// it can miss a match, when `after` can come right after it.
fn conflict(
    facts: &Facts<'_>,
    rule_id: RuleId,
    expression: &Expression,
    after: &Follow,
) -> Option<Conflict> {
    let conflict = |span, shorter, characters: CharacterSet| {
        (!characters.is_empty()).then_some(Conflict {
            rule_id,
            span,
            shorter,
            characters,
        })
    };
    let following =
        |characters: CharacterSet| characters.intersection(&after.characters);
    match &expression.kind {
        ExpressionKind::OptionalSequence(sequence)
        | ExpressionKind::RepeatedSequence(sequence) => {
            conflict(expression.span, None, following(facts.first(sequence)))
        }
        ExpressionKind::DefinitionsList(alternatives) => alternatives
            .iter()
            .enumerate()
            .find_map(|(index, earlier)| {
                alternatives[index + 1..].iter().find_map(|later| {
                    let past_earlier =
                        following(goes_on(facts, earlier, later));
                    conflict(later.span, Some(earlier.span), past_earlier)
                        .or_else(|| {
                            let past_later = goes_on(facts, later, earlier);
                            let characters = following(past_later);
                            conflict(
                                earlier.span,
                                Some(later.span),
                                characters,
                            )
                        })
                })
            }),
        _ => None,
    }
}

/// Returns the characters with which a match of `longer` can go on from where
/// a match of `shorter` ends, when that match is the start of this one.
fn goes_on(
    facts: &Facts<'_>,
    shorter: &Expression,
    longer: &Expression,
) -> CharacterSet {
    let (mut shorter, mut longer) = (Rest::new(shorter), Rest::new(longer));
    // Both match the same text for as long as they start with the same
    // character.
    while let (Some(first), Some(other)) =
        (shorter.character(), longer.character())
    {
        if first != other {
            break;
        }
        shorter = shorter.after_character();
        longer = longer.after_character();
    }
    let mut characters = if shorter.is_nullable(facts) {
        longer.first(facts)
    } else {
        CharacterSet::default()
    };
    if !shorter
        .first(facts)
        .intersection(&longer.first(facts))
        .is_empty()
    {
        characters = characters.union(&longer.inner(facts));
    }
    characters
}

/// What is left of a sequence to match: the rest of a text it starts with, and
/// the expressions after it.
#[derive(Clone, Copy)]
struct Rest<'e> {
    text: &'e str,
    steps: &'e [Expression],
}

impl<'e> Rest<'e> {
    fn new(expression: &'e Expression) -> Self {
        Self {
            text: "",
            steps: expression.steps(),
        }
        .with_text()
    }

    /// Returns the rest with the text of the terminal strings it starts with
    /// read into `text`, when it has none left.
    fn with_text(mut self) -> Self {
        while self.text.is_empty() {
            let [first, steps @ ..] = self.steps else {
                break;
            };
            let ExpressionKind::TerminalString(text) = &first.kind else {
                break;
            };
            self.text = text;
            self.steps = steps;
        }
        self
    }

    /// Returns the character the rest always starts with, if it is text.
    fn character(&self) -> Option<char> {
        self.text.chars().next()
    }

    /// Returns the rest after the character it always starts with.
    fn after_character(self) -> Self {
        let mut characters = self.text.chars();
        characters.next();
        Self {
            text: characters.as_str(),
            ..self
        }
        .with_text()
    }

    fn is_nullable(&self, facts: &Facts<'_>) -> bool {
        self.text.is_empty()
            && self.steps.iter().all(|step| facts.is_nullable(step))
    }

    /// Returns the characters the rest can start with.
    fn first(&self, facts: &Facts<'_>) -> CharacterSet {
        match self.character() {
            Some(character) => CharacterSet::single(character),
            None => self
                .steps
                .iter()
                .rev()
                .fold(CharacterSet::default(), |after, step| {
                    facts.first_before(step, &after)
                }),
        }
    }

    /// Returns the characters a match of the rest can hold after its first, as
    /// [`Facts::inner`] does.
    fn inner(&self, facts: &Facts<'_>) -> CharacterSet {
        let text = self
            .text
            .chars()
            .skip(1)
            .fold(CharacterSet::default(), |set, character| {
                set.union(&CharacterSet::single(character))
            });
        self.steps
            .iter()
            .enumerate()
            .fold(text, |set, (index, step)| {
                let set = set.union(&facts.inner(step));
                if index == 0 && self.text.is_empty() {
                    set
                } else {
                    set.union(&facts.first(step))
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::build_valid;

    fn general(source: &str) -> Vec<bool> {
        let grammar = build_valid(source);
        general_rules(&Facts::new(&grammar))
    }

    #[test]
    fn finds_an_alternative_that_goes_on_with_what_comes_after_it() {
        assert_eq!(general("s = ('x' | 'x', 'y'), 'y';"), [true]);
        assert_eq!(general("s = ('x' | 'x', 'y'), 'z';"), [false]);
        assert_eq!(general("s = ('xy' | 'x', 'z'), 'y';"), [false]);
    }

    #[test]
    fn finds_a_sequence_that_starts_like_what_comes_after_it() {
        assert_eq!(general("s = {'x'}, 'x';"), [true]);
        assert_eq!(general("s = ['x'], 'y';"), [false]);
        assert_eq!(general("s = [{'x'}], 'y';"), [false]);
    }

    #[test]
    fn finds_rules_that_reach_a_rule_that_needs_every_way() {
        let source = "s = word, 'fy'; word = keyword | name; keyword = 'if'; \
                      name = letter, {letter}; letter = ? U+0061..U+007A ?;";
        assert_eq!(general(source), [true, true, false, true, false]);
        let source = "s = word, ' '; word = keyword | name; keyword = 'if'; \
                      name = letter, {letter}; letter = ? U+0061..U+007A ?;";
        assert_eq!(general(source), [false; 5]);
    }

    #[test]
    fn leaves_a_greedy_rule_and_what_reaches_it_through_one() {
        let source = "s = word, 'fy'; word = keyword | name; keyword = 'if'; \
                      (* greedy *) name = letter, {letter}; \
                      letter = ? U+0061..U+007A ?;";
        assert_eq!(general(source), [true, true, false, false, false]);
        let source = "(* greedy *) s = a, 'x'; a = {'x'}, 'x';";
        assert_eq!(general(source), [false, true]);
    }
}
