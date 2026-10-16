//! Exactness tells where a parser that takes one way through each choice as it
//! comes to it still matches every input the grammar does.
//!
//! Such a parser takes the alternative that matches the most, or the first
//! that matches in a rule that takes the first match, and takes or repeats a
//! sequence whenever it can. It leaves out a way that the rest of the input
//! needs only where that way ends sooner, and the rest goes on from there with
//! a character the way it took matched instead. In a rule that takes the first
//! match, it also leaves out a later alternative that goes on from where an
//! earlier one ends. Where neither can happen, no way it leaves out could have
//! matched the input.

use crate::{
    analysis::{
        character_set::CharacterSet,
        facts::{Facts, Follow},
    },
    ir::{Expression, ExpressionKind, RuleId, reachable},
};

/// Returns which rules of the grammar `facts` answers about need code that
/// tries every way through them. Those are the rules with a choice that a
/// parser taking one way as it comes can get wrong, and every rule that
/// reaches one, since only what comes after a rule tells which way the rest of
/// the input needs.
pub(crate) fn general_rules(facts: &Facts<'_>) -> Vec<bool> {
    let rules = &facts.grammar().syntax_rules;
    let mut inexact = Vec::new();
    let mut callers = vec![Vec::new(); rules.len()];
    for (position, rule) in rules.iter().enumerate() {
        let mut exact = true;
        facts.walk_rule(RuleId(position), &mut |expression, after| {
            exact &= is_exact(facts, rule.first_match, expression, after);
        });
        if !exact {
            inexact.push(RuleId(position));
        }
        rule.expression
            .visit_meta_identifiers(&mut |RuleId(target), _| {
                callers[target].push(RuleId(position));
            });
    }
    reachable(rules.len(), inexact, |RuleId(position), pending| {
        pending.extend(&callers[position]);
    })
}

/// Returns whether a parser that takes one way through `expression` as it
/// comes to it never leaves out one the rest of the input needs, when what can
/// come right after it is `after`.
fn is_exact(
    facts: &Facts<'_>,
    first_match: bool,
    expression: &Expression,
    after: &Follow,
) -> bool {
    let follows = |characters: CharacterSet| {
        !characters.intersection(&after.characters).is_empty()
    };
    match &expression.kind {
        ExpressionKind::OptionalSequence(sequence)
        | ExpressionKind::RepeatedSequence(sequence) => {
            !follows(facts.first(sequence))
        }
        ExpressionKind::DefinitionsList(alternatives) => {
            alternatives.iter().enumerate().all(|(index, earlier)| {
                alternatives[index + 1..].iter().all(|later| {
                    let past_earlier = goes_on(facts, earlier, later);
                    let past_later = goes_on(facts, later, earlier);
                    let takes_later = if first_match {
                        past_earlier.is_empty()
                    } else {
                        !follows(past_earlier)
                    };
                    takes_later && !follows(past_later)
                })
            })
        }
        _ => true,
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
    fn finds_an_earlier_alternative_that_a_first_match_takes_too_soon() {
        assert_eq!(
            general(
                "items = item, {';', item}; \
                 (* first match *) item = 'ab' | 'a', {'b'};"
            ),
            [true, true]
        );
        assert_eq!(general("(* first match *) s = 'xy' | 'x', 'z';"), [false]);
        assert_eq!(general("(* first match *) s = 'x', 'y' | 'x';"), [false]);
    }
}
