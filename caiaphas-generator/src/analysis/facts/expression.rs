//! How each fact about an expression follows from the same facts about the
//! expressions inside it and the rules it references.

use super::Facts;
use crate::{
    analysis::character_set::CharacterSet,
    ir::{Expression, ExpressionKind, Rule, RuleId},
};

pub(super) fn is_nullable(expression: &Expression, rules: &[bool]) -> bool {
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => expressions
            .iter()
            .all(|expression| is_nullable(expression, rules)),
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .any(|expression| is_nullable(expression, rules)),
        ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::Not(_)
        | ExpressionKind::EmptySequence => true,
        ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        }
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => is_nullable(expression, rules),
        ExpressionKind::MetaIdentifier(RuleId(position)) => rules[*position],
        ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_) => false,
    }
}

pub(super) fn ends_once(
    expression: &Expression,
    rules: &[bool],
    characters: &[Option<CharacterSet>],
) -> bool {
    if single_characters(expression, characters).is_some() {
        return true;
    }
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => expressions
            .iter()
            .all(|expression| ends_once(expression, rules, characters)),
        ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        }
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => ends_once(expression, rules, characters),
        ExpressionKind::MetaIdentifier(RuleId(position)) => rules[*position],
        ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_)
        | ExpressionKind::Not(_)
        | ExpressionKind::EmptySequence => true,
        ExpressionKind::DefinitionsList(_)
        | ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_) => false,
    }
}

pub(super) fn is_productive(expression: &Expression, rules: &[bool]) -> bool {
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => expressions
            .iter()
            .all(|expression| is_productive(expression, rules)),
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .any(|expression| is_productive(expression, rules)),
        ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_)
        | ExpressionKind::Not(_)
        | ExpressionKind::EmptySequence => true,
        ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        }
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => is_productive(expression, rules),
        ExpressionKind::MetaIdentifier(RuleId(position)) => rules[*position],
    }
}

pub(super) fn first(
    expression: &Expression,
    nullable: &[bool],
    rules: &[CharacterSet],
    characters: &[Option<CharacterSet>],
) -> CharacterSet {
    let first = |expression| first(expression, nullable, rules, characters);
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => {
            let mut set = CharacterSet::default();
            for expression in expressions {
                set = set.union(&first(expression));
                if !is_nullable(expression, nullable) {
                    break;
                }
            }
            set
        }
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .fold(CharacterSet::default(), |set, expression| {
                set.union(&first(expression))
            }),
        ExpressionKind::OptionalSequence(expression)
        | ExpressionKind::RepeatedSequence(expression)
        | ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        } => first(expression),
        ExpressionKind::SyntacticTerm {
            syntactic_factor, ..
        } => single_characters(expression, characters)
            .unwrap_or_else(|| first(syntactic_factor)),
        ExpressionKind::MetaIdentifier(RuleId(position)) => {
            rules[*position].clone()
        }
        ExpressionKind::TerminalString(string) => string
            .chars()
            .next()
            .map_or_else(CharacterSet::default, CharacterSet::single),
        ExpressionKind::SpecialSequence(range) => {
            CharacterSet::from_range(range.clone())
        }
        ExpressionKind::Not(_) | ExpressionKind::EmptySequence => {
            CharacterSet::default()
        }
    }
}

/// Returns what [`Facts::passing`] does, given the same for each rule in
/// `rules` and the single characters of each in `characters`.
pub(super) fn passing(
    expression: &Expression,
    rules: &[CharacterSet],
    characters: &[Option<CharacterSet>],
) -> CharacterSet {
    let passing = |expression| passing(expression, rules, characters);
    let every = || CharacterSet::from_range('\0'..=char::MAX);
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => {
            expressions.iter().fold(every(), |set, expression| {
                set.intersection(&passing(expression))
            })
        }
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .fold(CharacterSet::default(), |set, expression| {
                set.union(&passing(expression))
            }),
        ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::EmptySequence => every(),
        ExpressionKind::Not(ahead) => {
            match single_characters(ahead, characters) {
                Some(taken) => every().difference(&taken),
                None => every(),
            }
        }
        ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        }
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => passing(expression),
        ExpressionKind::MetaIdentifier(RuleId(position)) => {
            rules[*position].clone()
        }
        ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_) => CharacterSet::default(),
    }
}

/// Returns whether `expression` holds a `not`, or reads a rule that looks
/// ahead by `rules`.
pub(super) fn looks_ahead(expression: &Expression, rules: &[bool]) -> bool {
    let mut found = false;
    expression.visit(&mut |expression| match &expression.kind {
        ExpressionKind::Not(_) => found = true,
        ExpressionKind::MetaIdentifier(RuleId(position)) => {
            found |= rules[*position];
        }
        _ => {}
    });
    found
}

/// Returns what [`Facts::inner`] does, given the same for each rule in
/// `rules`.
pub(super) fn inner(
    expression: &Expression,
    facts: &Facts<'_>,
    rules: &[CharacterSet],
) -> CharacterSet {
    let inner = |expression| inner(expression, facts, rules);
    match &expression.kind {
        ExpressionKind::TerminalString(string) => string
            .chars()
            .skip(1)
            .fold(CharacterSet::default(), |set, character| {
                set.union(&CharacterSet::single(character))
            }),
        ExpressionKind::SingleDefinition(expressions) => expressions
            .iter()
            .enumerate()
            .fold(CharacterSet::default(), |set, (index, expression)| {
                let set = set.union(&inner(expression));
                if index == 0 {
                    set
                } else {
                    set.union(&facts.first(expression))
                }
            }),
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .fold(CharacterSet::default(), |set, expression| {
                set.union(&inner(expression))
            }),
        ExpressionKind::OptionalSequence(expression)
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => inner(expression),
        ExpressionKind::RepeatedSequence(expression)
        | ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        } => inner(expression).union(&facts.first(expression)),
        ExpressionKind::MetaIdentifier(RuleId(position)) => {
            rules[*position].clone()
        }
        ExpressionKind::SpecialSequence(_)
        | ExpressionKind::Not(_)
        | ExpressionKind::EmptySequence => CharacterSet::default(),
    }
}

/// What is known so far about the single characters of a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SingleCharacterSearch {
    Unvisited,
    /// Being worked out, so reaching it again means recursion.
    Visiting,
    Known(Option<CharacterSet>),
}

/// Returns the single characters of the rule at `position`, working out those
/// of the rules it references first.
///
/// A recursive rule gets no answer, and neither does any rule that references
/// one. Nor does a converted rule, since its type can reject a character.
pub(super) fn rule_single_characters(
    rules: &[Rule],
    states: &mut [SingleCharacterSearch],
    position: usize,
) -> Option<CharacterSet> {
    match &states[position] {
        SingleCharacterSearch::Known(characters) => return characters.clone(),
        SingleCharacterSearch::Visiting => return None,
        SingleCharacterSearch::Unvisited => {}
    }
    if rules[position].converts_to.is_some() {
        states[position] = SingleCharacterSearch::Known(None);
        return None;
    }
    states[position] = SingleCharacterSearch::Visiting;
    let characters = single_characters_with(
        &rules[position].expression,
        &mut |RuleId(target)| rule_single_characters(rules, states, target),
    );
    states[position] = SingleCharacterSearch::Known(characters.clone());
    characters
}

/// Returns the single characters of `expression`, given those of each rule in
/// `characters`.
pub(super) fn single_characters(
    expression: &Expression,
    characters: &[Option<CharacterSet>],
) -> Option<CharacterSet> {
    single_characters_with(expression, &mut |RuleId(position)| {
        characters[position].clone()
    })
}

pub(super) fn single_characters_with(
    expression: &Expression,
    rule: &mut impl FnMut(RuleId) -> Option<CharacterSet>,
) -> Option<CharacterSet> {
    match &expression.kind {
        ExpressionKind::TerminalString(string) => {
            let mut chars = string.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(CharacterSet::single(c)),
                _ => None,
            }
        }
        ExpressionKind::SpecialSequence(range) => {
            Some(CharacterSet::from_range(range.clone()))
        }
        ExpressionKind::DefinitionsList(expressions) => {
            let mut set = CharacterSet::default();
            for expression in expressions {
                set = set.union(&single_characters_with(expression, rule)?);
            }
            Some(set)
        }
        ExpressionKind::MetaIdentifier(rule_id) => rule(*rule_id),
        ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } => {
            let factor = single_characters_with(syntactic_factor, rule)?;
            let exception = single_characters_with(syntactic_exception, rule)?;
            Some(factor.difference(&exception))
        }
        ExpressionKind::SingleDefinition(_)
        | ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::SyntacticFactor { .. }
        | ExpressionKind::Not(_)
        | ExpressionKind::EmptySequence => None,
    }
}
