//! Traces tell where the type of a rule loses what its matches tell apart: in
//! a field that gathers places of a sequence that hold a rule a different
//! number of times each, and in a choice whose alternatives hold the same
//! rules.

use super::{
    Shape,
    counts::{Occurrences, labels, occurrences},
    shapes,
};
use crate::{
    ir::{Expression, ExpressionKind, Grammar, RuleId},
    span::Span,
};

/// A rule that places of one sequence hold a different number of times each,
/// in a struct whose field for it gathers every match.
pub(crate) struct Gathering {
    /// The rule of the struct.
    pub(crate) rule: RuleId,
    /// The rule the field holds, or the label that names it.
    pub(crate) held: RuleId,
    /// The sequence.
    pub(crate) sequence: Span,
    /// The parts of the sequence that hold the rule.
    pub(crate) places: Vec<Span>,
}

/// Returns each rule that places of a sequence hold a different number of
/// times each, in each struct of `grammar`. A list such as `item, {',', item}`
/// is one place, since the repetitions after the first place repeat it.
pub(crate) fn gatherings(grammar: &Grammar) -> Vec<Gathering> {
    let labels = labels(grammar);
    let mut gatherings = Vec::new();
    for (position, shape) in shapes(grammar).iter().enumerate() {
        if !matches!(shape, Some(Shape::Struct(_))) {
            continue;
        }
        let rule = &grammar.syntax_rules[position];
        rule.expression.visit(&mut |expression| {
            let ExpressionKind::SingleDefinition(steps) = &expression.kind
            else {
                return;
            };
            let held: Vec<_> = steps
                .iter()
                .map(|step| occurrences(grammar, &labels, step))
                .collect();
            let mut seen = Vec::new();
            for &(rule_id, _) in held.iter().flatten() {
                if seen.contains(&rule_id) {
                    continue;
                }
                seen.push(rule_id);
                let places: Vec<(&Expression, Occurrences)> = steps
                    .iter()
                    .zip(&held)
                    .filter_map(|(step, held)| {
                        let &(_, count) = held
                            .iter()
                            .find(|&&(other, _)| other == rule_id)?;
                        Some((step, count))
                    })
                    .collect();
                let Some(((_, first), later)) = places.split_first() else {
                    continue;
                };
                let list = later.iter().all(|(step, _)| {
                    matches!(step.kind, ExpressionKind::RepeatedSequence(_))
                });
                let alike = later.iter().all(|(_, count)| count == first);
                if later.is_empty() || list || alike {
                    continue;
                }
                gatherings.push(Gathering {
                    rule: RuleId(position),
                    held: rule_id,
                    sequence: expression.span,
                    places: places.iter().map(|(step, _)| step.span).collect(),
                });
            }
        });
    }
    gatherings
}

/// A choice in a struct whose alternatives hold the same rules as often, so no
/// field tells which of them matched.
pub(crate) struct Untraced {
    /// The rule of the struct.
    pub(crate) rule: RuleId,
    pub(crate) choice: Span,
    /// The alternatives that hold what another one holds.
    pub(crate) alternatives: Vec<Span>,
}

/// Returns each choice in a struct of `grammar` that stands between places
/// that hold rules, as an operator stands between its operands, when its
/// alternatives are made of terminal strings and rules and hold the same rules
/// as often, though they are written differently.
pub(crate) fn untraced_choices(grammar: &Grammar) -> Vec<Untraced> {
    let labels = labels(grammar);
    let mut found = Vec::new();
    for (position, shape) in shapes(grammar).iter().enumerate() {
        if !matches!(shape, Some(Shape::Struct(_))) {
            continue;
        }
        let mut walk = Walk {
            grammar,
            labels: &labels,
            rule: RuleId(position),
            found: &mut found,
        };
        walk.expression(
            &grammar.syntax_rules[position].expression,
            false,
            false,
        );
    }
    found
}

/// Walks the expression of the rule of a struct, knowing whether parts that
/// hold rules can come before and after each part.
struct Walk<'a> {
    grammar: &'a Grammar,
    labels: &'a [Option<RuleId>],
    rule: RuleId,
    found: &'a mut Vec<Untraced>,
}

impl Walk<'_> {
    /// Returns whether `expression` holds a rule with a type.
    fn holds(&self, expression: &Expression) -> bool {
        !occurrences(self.grammar, self.labels, expression).is_empty()
    }

    /// Walks `expression`, where parts that hold rules can come `before` and
    /// `after` it.
    fn expression(
        &mut self,
        expression: &Expression,
        before: bool,
        after: bool,
    ) {
        match &expression.kind {
            ExpressionKind::SingleDefinition(steps) => {
                let holding: Vec<bool> =
                    steps.iter().map(|step| self.holds(step)).collect();
                for (index, step) in steps.iter().enumerate() {
                    let before = before || holding[..index].contains(&true);
                    let after = after || holding[index + 1..].contains(&true);
                    self.expression(step, before, after);
                }
            }
            ExpressionKind::DefinitionsList(alternatives) => {
                if before && after {
                    self.choice(expression, alternatives);
                }
                for alternative in alternatives {
                    self.expression(alternative, before, after);
                }
            }
            ExpressionKind::OptionalSequence(body)
            | ExpressionKind::SyntacticTerm {
                syntactic_factor: body,
                ..
            } => self.expression(body, before, after),
            // Each round comes before and after another.
            ExpressionKind::RepeatedSequence(body)
            | ExpressionKind::SyntacticFactor {
                syntactic_primary: body,
                ..
            } => {
                let again = self.holds(body);
                self.expression(body, before || again, after || again);
            }
            ExpressionKind::MetaIdentifier(_)
            | ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::Not(_)
            | ExpressionKind::EmptySequence => {}
        }
    }

    /// Notes `choice` when its alternatives are made of terminal strings and
    /// rules and some hold the same rules as often as another, though written
    /// differently.
    fn choice(&mut self, choice: &Expression, alternatives: &[Expression]) {
        let mut plain = true;
        for alternative in alternatives {
            alternative.visit(&mut |part| {
                plain &= !matches!(
                    part.kind,
                    ExpressionKind::SpecialSequence(_)
                        | ExpressionKind::Not(_)
                );
            });
        }
        if !plain {
            return;
        }
        let held: Vec<Vec<(RuleId, Occurrences)>> = alternatives
            .iter()
            .map(|alternative| {
                let mut held =
                    occurrences(self.grammar, self.labels, alternative);
                held.sort_by_key(|&(RuleId(rule), _)| rule);
                held
            })
            .collect();
        let alike: Vec<Span> = alternatives
            .iter()
            .enumerate()
            .filter(|&(index, alternative)| {
                alternatives.iter().enumerate().any(|(other, written)| {
                    other != index
                        && held[other] == held[index]
                        && !written.is_written_like(alternative)
                })
            })
            .map(|(_, alternative)| alternative.span)
            .collect();
        if !alike.is_empty() {
            self.found.push(Untraced {
                rule: self.rule,
                choice: choice.span,
                alternatives: alike,
            });
        }
    }
}
