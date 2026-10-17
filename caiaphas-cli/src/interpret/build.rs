//! Building adds the nodes of a match of a rule that tries every way through
//! it, from where it starts to the end the rest of the input goes on from, as
//! the function generated for it does.

use caiaphas::__private::{Ends, Parser, Width};
use caiaphas_generator::plan::{Kind, Part};

use super::Program;

/// A step of a sequence, as the parser splits a match among the steps.
type Step<'s, const R: bool, W> =
    &'s dyn Fn(&mut Parser<'_, R, W>, usize) -> Ends;

impl Program {
    /// Builds the node of the rule numbered `id` from `position` to `end`, for
    /// a rule that tries every way, as its function that builds it does.
    pub(super) fn build_rule<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        id: usize,
        position: usize,
        end: usize,
    ) -> Option<usize> {
        // A rule that recovered builds its node from where it went on.
        let position = if self.plan_of(id).recovers {
            parser.recovered(&self.rules[id], position)
        } else {
            position
        };
        let start = parser.start_rule(position);
        self.build(parser, id, &self.plan_of(id).expression, position, end);
        parser.built(&self.rules[id], start, end)
    }

    /// Adds the nodes of the rules matched inside `part`, in its match from
    /// `position` to `end`.
    fn build<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        position: usize,
        end: usize,
    ) {
        if !part.builds_nodes {
            return;
        }
        match &part.kind {
            Kind::Sequence(steps) => {
                self.build_steps(parser, rule, steps, position, end);
            }
            Kind::Choice(alternatives) => {
                // Alternatives that end alike tie, and the first written of
                // them wins.
                let (last, earlier) = alternatives
                    .split_last()
                    .expect("a choice has alternatives");
                for alternative in earlier {
                    if self
                        .ends_branch(parser, rule, alternative, position)
                        .contains(end)
                    {
                        self.build(parser, rule, alternative, position, end);
                        return;
                    }
                }
                self.build(parser, rule, last, position, end);
            }
            Kind::Optional(sequence) => {
                if sequence.nullable {
                    if self
                        .ends(parser, rule, sequence, position)
                        .contains(end)
                    {
                        self.build(parser, rule, sequence, position, end);
                    }
                } else if end != position {
                    self.build(parser, rule, sequence, position, end);
                }
            }
            Kind::Repeat(sequence) => {
                let mut position = position;
                let rounds =
                    parser.rounds_to(position, end, |parser, position| {
                        self.ends_branch(parser, rule, sequence, position)
                    });
                for next in rounds {
                    self.build(parser, rule, sequence, position, next);
                    position = next;
                }
            }
            Kind::Times(times, primary) => {
                let mut position = position;
                for left in (0..*times).rev() {
                    let candidates =
                        self.part_ends(parser, rule, primary, position);
                    let next = parser.choose(&candidates, |parser, next| {
                        let ends = parser.times(
                            Ends::one(next),
                            left,
                            |parser, position| {
                                self.part_ends(parser, rule, primary, position)
                            },
                        );
                        ends.contains(end)
                    });
                    self.build(parser, rule, primary, position, next);
                    position = next;
                }
            }
            Kind::Except { factor, .. } => {
                self.build(parser, rule, factor, position, end);
            }
            Kind::Rule(id) => {
                if self.plan_of(*id).general {
                    let node = self.build_rule(parser, *id, position, end);
                    parser.add_node(node);
                } else {
                    let found = self.rule(parser, *id, position);
                    parser.matched(found);
                }
            }
            _ => {}
        }
    }

    /// Adds the nodes of `steps`, matched one after another from `position` to
    /// `end`.
    fn build_steps<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        steps: &'static [Part],
        position: usize,
        end: usize,
    ) {
        let Some(last) = steps.iter().rposition(|step| step.builds_nodes)
        else {
            return;
        };
        // Up to the last step that builds a node, steps that each have one end
        // go on from it without choosing.
        let chained = (last + 1).min(steps.len() - 1);
        if steps[..chained].iter().all(|step| step.has_one_end) {
            let mut position = position;
            for (index, step) in steps.iter().enumerate().take(last + 1) {
                if index + 1 == steps.len() {
                    self.build(parser, rule, step, position, end);
                    break;
                }
                let middle =
                    self.part_ends(parser, rule, step, position).only();
                self.build(parser, rule, step, position, middle);
                if index < last {
                    position = middle;
                }
            }
            return;
        }
        let parts: Vec<_> = steps
            .iter()
            .map(|step| {
                move |parser: &mut Parser<'_, R, W>, position: usize| {
                    self.part_ends(parser, rule, step, position)
                }
            })
            .collect();
        let references: Vec<Step<'_, R, W>> =
            parts.iter().map(|part| part as Step<'_, R, W>).collect();
        let split = parser.split(&references, position, end);
        for (index, step) in steps.iter().enumerate() {
            if step.builds_nodes {
                let start = index
                    .checked_sub(1)
                    .map_or(position, |before| split[before]);
                self.build(parser, rule, step, start, split[index]);
            }
        }
    }
}
