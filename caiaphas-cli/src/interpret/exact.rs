//! Exact matching runs a rule that tries every way through it, as the code
//! generated for it does: one function tells where its matches can end, and
//! another builds the match the rest of the input goes on from.

use caiaphas::__private::{Ends, Parser, Width};
use caiaphas_generator::plan::{Kind, Part};

use super::{
    Program, contains, expected, set_except, set_parts, sets_apart, single,
    skips,
};

/// A step of a sequence, as the parser splits a match among the steps.
type Step<'s, const R: bool, W> =
    &'s dyn Fn(&mut Parser<'_, R, W>, usize) -> Ends;

impl Program {
    /// Returns where the matches of the rule numbered `id` can end at
    /// `position`, trying every way, as its function that tells them does.
    pub(super) fn ends_rule<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        id: usize,
        position: usize,
    ) -> Ends {
        let plan = self.plan_of(id);
        if plan.recursive {
            if !parser.enter(position) {
                return Ends::none();
            }
            let ends = self.rule_ends(parser, id, position);
            parser.leave();
            return ends;
        }
        self.rule_ends(parser, id, position)
    }

    fn rule_ends<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        id: usize,
        position: usize,
    ) -> Ends {
        let rule = &self.rules[id];
        if let Some(ends) = parser.recall_ends(rule, position) {
            return ends;
        }
        let start = parser.start_rule(position);
        let mut ends =
            self.ends(parser, id, &self.plan_of(id).expression, position);
        if self.plan_of(id).converts_to.is_some() {
            ends.retain(|end| self.converts(parser, id, position, end));
        }
        parser.finish_ends(rule, start, ends)
    }

    /// Returns where `part` of the rule numbered `rule` can end at `position`,
    /// as the code of a rule that tries every way does.
    fn ends<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        position: usize,
    ) -> Ends {
        if let Some(set) = &part.set {
            return Ends::of(single(parser, part, set, position));
        }
        let first_match = self.plan_of(rule).first_match;
        let keeps_order = !first_match && part.keeps_order;
        let ways = match &part.kind {
            Kind::Sequence(steps) => {
                let unordered = !first_match && !keeps_order;
                let mut ends = Ends::one(position);
                for step in steps {
                    ends = self.step(parser, rule, step, &ends, unordered);
                }
                ends
            }
            Kind::Choice(alternatives) => {
                let mut ends = Ends::none();
                for alternative in alternatives {
                    let found =
                        self.ends_branch(parser, rule, alternative, position);
                    if keeps_order {
                        ends.merge(found);
                    } else {
                        ends.extend(found);
                    }
                }
                ends
            }
            Kind::Optional(sequence) => {
                let mut ends =
                    self.ends_branch(parser, rule, sequence, position);
                ends.add(position);
                ends
            }
            Kind::Repeat(sequence) => {
                if let Some(set) = set_parts(sequence) {
                    let expected = expected(&set.expected);
                    return parser.span_ends(
                        position,
                        contains(&set.ranges),
                        &expected,
                    );
                }
                parser.rounds(position, |parser, position| {
                    self.ends_branch(parser, rule, sequence, position)
                })
            }
            Kind::Times(times, primary) => parser.times(
                Ends::one(position),
                *times,
                |parser, position| {
                    self.part_ends(parser, rule, primary, position)
                },
            ),
            Kind::Except { factor, exception } => {
                if let Some((set, excludes)) = sets_apart(part, factor) {
                    return Ends::of(set_except(
                        parser, set, excludes, position,
                    ));
                }
                return self.except_ends(
                    parser, rule, part, factor, exception, position,
                );
            }
            Kind::Rule(id) => {
                if self.plan_of(*id).general {
                    return self.ends_rule(parser, *id, position);
                }
                let found = self.rule(parser, *id, position);
                return Ends::of(found.map(|(end, _)| end));
            }
            Kind::Text(text) => {
                return Ends::of(parser.text(position, text));
            }
            Kind::Range(first, last) => {
                return Ends::of(parser.range(position, *first, *last));
            }
            Kind::Empty => return Ends::one(position),
            _ => unreachable!("a plan holds no other part"),
        };
        if first_match || keeps_order {
            ways
        } else {
            ways.longest_first()
        }
    }

    /// Returns where what `factor` matches and `exception` does not can end,
    /// for the exception `part`.
    fn except_ends<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        factor: &'static Part,
        exception: &'static Part,
        position: usize,
    ) -> Ends {
        let mut ends = self.ends(parser, rule, factor, position);
        let constant = &self.rules[rule];
        if part.settled {
            if let Some(furthest) = ends.iter().max() {
                let excluded = {
                    let mut within = parser.within(furthest);
                    self.ends(&mut within, rule, exception, position)
                };
                ends.retain(|end| {
                    let excluded = excluded.contains(end);
                    if excluded {
                        parser.excluded(constant, position, end);
                    }
                    !excluded
                });
            }
            return ends;
        }
        ends.retain(|end| {
            let excluded = {
                let mut within = parser.within(end);
                self.ends(&mut within, rule, exception, position)
                    .contains(end)
            };
            if excluded {
                parser.excluded(constant, position, end);
            }
            !excluded
        });
        ends
    }

    /// Returns where `step` of a sequence can end after each of `ends`.
    fn step<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        step: &'static Part,
        ends: &Ends,
        unordered: bool,
    ) -> Ends {
        let Kind::Repeat(sequence) = &step.kind else {
            return parser.then(ends, |parser, position| {
                self.part_ends(parser, rule, step, position)
            });
        };
        if let Some(set) = set_parts(sequence) {
            let expected = expected(&set.expected);
            return parser.then_span(ends, contains(&set.ranges), &expected);
        }
        if unordered {
            return parser.then_rounds(ends, |parser, position| {
                self.ends_branch(parser, rule, sequence, position)
            });
        }
        parser.then(ends, |parser, position| {
            self.part_ends(parser, rule, step, position)
        })
    }

    /// Returns where `part` can end from `position`, as a part of a sequence.
    fn part_ends<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        position: usize,
    ) -> Ends {
        if let Kind::Empty = part.kind {
            return Ends::one(position);
        }
        self.ends(parser, rule, part, position)
    }

    /// Returns where `part` can end as a branch, skipped by a parse that only
    /// matches where the next character is none it can start with.
    fn ends_branch<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        position: usize,
    ) -> Ends {
        if skips::<R, W>(parser, part, part.set.is_some(), position) {
            return Ends::none();
        }
        self.ends(parser, rule, part, position)
    }

    /// Builds the node of the rule numbered `id` from `position` to `end`, for
    /// a rule that tries every way, as its function that builds it does.
    pub(super) fn build_rule<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        id: usize,
        position: usize,
        end: usize,
    ) -> Option<usize> {
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
