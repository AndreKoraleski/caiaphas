//! Exact matching runs a rule that tries every way through it, as the code
//! generated for it does. One function tells where its matches can end, and
//! another, in `build`, builds the match the rest of the input goes on from.

use std::ptr;

use caiaphas::__private::{Ends, Parser, Width};
use caiaphas_generator::plan::{Kind, Part};

use super::{
    Program, again_ends, contains, expected, recovering, set_except,
    set_parts, sets_apart, single, skips,
};

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
        let ends = parser.finish_ends(rule, start, ends);
        // A rule that recovers goes on from where a parse that recovers tells
        // it to, when it fails.
        if ends.first().is_some() || !self.plan_of(id).recovers {
            return ends;
        }
        recovering(id, || parser.recover_ends(rule, start, again_ends::<R, W>))
    }

    /// Returns where `part` of the rule numbered `rule` can end at `position`,
    /// as the code of a rule that tries every way does.
    pub(super) fn ends<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        position: usize,
    ) -> Ends {
        if let Some(set) = &part.set {
            return Ends::of(single(parser, part, set, position));
        }
        let ways = match &part.kind {
            Kind::Sequence(steps) => {
                let mut ends = Ends::one(position);
                for step in steps {
                    ends = self.step(parser, rule, step, &ends);
                }
                ends
            }
            Kind::Choice(alternatives) => {
                let mut ends = Ends::none();
                for alternative in alternatives {
                    ends.extend(self.ends_branch(
                        parser,
                        rule,
                        alternative,
                        position,
                    ));
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
                if part.before_middle {
                    return self.before_middle(
                        parser, rule, factor, exception, position,
                    );
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
        ways.longest_first()
    }

    /// Returns where the run of characters `factor` matches can end before the
    /// first match inside it of the middle of `exception`, for an exception
    /// that takes away any run holding one.
    fn before_middle<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        factor: &'static Part,
        exception: &'static Part,
        position: usize,
    ) -> Ends {
        let (Kind::Repeat(run), Kind::Sequence(steps)) =
            (&factor.kind, &exception.kind)
        else {
            unreachable!("the factor is a run and the exception a sequence");
        };
        let set = set_parts(run).expect("a run is of a set");
        let middle = &steps[1..steps.len() - 1];
        parser.span_ends_before(
            &self.rules[rule],
            ptr::from_ref(factor) as usize,
            position,
            contains(&set.ranges),
            &expected(&set.expected),
            |within, position| {
                let mut ends = Ends::one(position);
                for step in middle {
                    ends = self.step(within, rule, step, &ends);
                }
                ends
            },
        )
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
        parser.then_rounds(ends, |parser, position| {
            self.ends_branch(parser, rule, sequence, position)
        })
    }

    /// Returns where `part` can end from `position`, as a part of a sequence.
    pub(super) fn part_ends<const R: bool, W: Width>(
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
    pub(super) fn ends_branch<const R: bool, W: Width>(
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
}
