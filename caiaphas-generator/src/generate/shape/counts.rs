//! Counts tell how many times an expression holds each rule with a type, and
//! which inline rules label the one rule they hold.

use super::MANY;
use crate::ir::{Expression, ExpressionKind, Grammar, RuleId};

/// Returns the rule each inline rule labels: the one rule it holds, when that
/// is a rule with a type. An inline rule that only holds a label passes the
/// label on instead, so no label holds another.
pub(crate) fn labels(grammar: &Grammar) -> Vec<Option<RuleId>> {
    let count = grammar.syntax_rules.len();
    let (mut labels, mut known) = (vec![None; count], vec![false; count]);
    for position in 0..count {
        label(grammar, &mut labels, &mut known, position);
    }
    labels
}

/// Returns whether the node of each rule stands for its children, as an inline
/// rule's does unless it labels a rule.
pub(crate) fn spliced(grammar: &Grammar) -> Vec<bool> {
    labels(grammar)
        .iter()
        .zip(&grammar.syntax_rules)
        .map(|(label, rule)| rule.inline && label.is_none())
        .collect()
}

/// Returns the rule the rule at `position` labels, working out the labels of
/// the inline rules it holds first. The dictionary makes sure no inline rule
/// holds itself through inline rules alone.
fn label(
    grammar: &Grammar,
    labels: &mut [Option<RuleId>],
    known: &mut [bool],
    position: usize,
) -> Option<RuleId> {
    if known[position] {
        return labels[position];
    }
    known[position] = true;
    let rule = &grammar.syntax_rules[position];
    if !rule.inline {
        return None;
    }
    rule.expression
        .visit_meta_identifiers(&mut |RuleId(read), _| {
            label(grammar, labels, known, read);
        });
    labels[position] = match occurrences(grammar, labels, &rule.expression)[..]
    {
        [(RuleId(held), _)] if labels[held].is_none() => Some(RuleId(held)),
        _ => None,
    };
    labels[position]
}

/// How many times an expression holds a rule, at least and at most, where
/// [`MANY`] stands for that many or more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Occurrences {
    pub(super) least: u8,
    pub(super) most: u8,
}

/// Exactly once.
pub(super) const ONCE: Occurrences = Occurrences { least: 1, most: 1 };

/// Returns how many times `expression` holds each rule with a type or each
/// label, as `labels` tells them, in the order the rules first appear.
pub(super) fn occurrences(
    grammar: &Grammar,
    labels: &[Option<RuleId>],
    expression: &Expression,
) -> Vec<(RuleId, Occurrences)> {
    let of = |expression| occurrences(grammar, labels, expression);
    match &expression.kind {
        ExpressionKind::MetaIdentifier(rule_id) => {
            let RuleId(position) = *rule_id;
            let rule = &grammar.syntax_rules[position];
            if rule.hidden {
                return Vec::new();
            }
            // The dictionary makes sure no inline rule holds itself through
            // inline rules alone.
            if rule.inline && labels[position].is_none() {
                return of(&rule.expression);
            }
            vec![(*rule_id, ONCE)]
        }
        ExpressionKind::SingleDefinition(expressions) => {
            expressions.iter().fold(Vec::new(), |held, expression| {
                combine(&held, &of(expression), |first, second| Occurrences {
                    least: first.least.saturating_add(second.least).min(MANY),
                    most: first.most.saturating_add(second.most).min(MANY),
                })
            })
        }
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .map(of)
            .reduce(|held, other| {
                combine(&held, &other, |first, second| Occurrences {
                    least: first.least.min(second.least),
                    most: first.most.max(second.most),
                })
            })
            .unwrap_or_default(),
        ExpressionKind::OptionalSequence(expression) => {
            scaled(of(expression), |count| Occurrences { least: 0, ..count })
        }
        ExpressionKind::RepeatedSequence(expression) => {
            scaled(of(expression), |_| Occurrences {
                least: 0,
                most: MANY,
            })
        }
        ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary,
        } => {
            let times = u8::try_from(*integer).unwrap_or(MANY);
            scaled(of(syntactic_primary), |count| Occurrences {
                least: count.least.saturating_mul(times).min(MANY),
                most: count.most.saturating_mul(times).min(MANY),
            })
        }
        ExpressionKind::SyntacticTerm {
            syntactic_factor, ..
        } => of(syntactic_factor),
        ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_)
        | ExpressionKind::Not(_)
        | ExpressionKind::EmptySequence => Vec::new(),
    }
}

/// Returns the rules of `first` and `second`, in the order they first appear,
/// with `merge` giving the count of a rule from both counts, where a rule that
/// one does not hold counts as held no times.
fn combine(
    first: &[(RuleId, Occurrences)],
    second: &[(RuleId, Occurrences)],
    merge: impl Fn(Occurrences, Occurrences) -> Occurrences,
) -> Vec<(RuleId, Occurrences)> {
    let none = Occurrences { least: 0, most: 0 };
    let count_in = |held: &[(RuleId, Occurrences)], rule: RuleId| {
        held.iter()
            .find(|(other, _)| *other == rule)
            .map_or(none, |&(_, count)| count)
    };
    let mut all: Vec<(RuleId, Occurrences)> = first
        .iter()
        .map(|&(rule, count)| (rule, merge(count, count_in(second, rule))))
        .collect();
    for &(rule, count) in second {
        if !first.iter().any(|&(other, _)| other == rule) {
            all.push((rule, merge(none, count)));
        }
    }
    all
}

fn scaled(
    held: Vec<(RuleId, Occurrences)>,
    scale: impl Fn(Occurrences) -> Occurrences,
) -> Vec<(RuleId, Occurrences)> {
    held.into_iter()
        .map(|(rule, count)| (rule, scale(count)))
        .collect()
}
