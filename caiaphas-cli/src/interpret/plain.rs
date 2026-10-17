//! Plain matching runs a rule that takes each way as it comes, as the code
//! generated for it does.

use caiaphas::__private::{Expected, Match, Parser, Width};
use caiaphas_generator::plan::{Kind, Part};

use super::{
    Branch, Program, accepts, again, branches, contains, expected, recovering,
    set_except, set_parts, sets_apart, single, skips, within,
};

impl Program {
    /// Matches the rule numbered `id`, taking each way as it comes.
    pub(super) fn plain_rule<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        id: usize,
        position: usize,
    ) -> Match {
        let (plan, rule) = (self.plan_of(id), &self.rules[id]);
        if plan.remembers {
            if let Some(found) = parser.recall(rule, position) {
                return found;
            }
        }
        // A parse that only matches needs no account of where a leaf starts.
        let leaf = plan.leaf && !plan.remembers && !plan.recovers;
        let start = (R || !leaf).then(|| parser.start_rule(position));
        let end = self.plain(parser, id, &plan.expression, position);
        let end = end.filter(|&end| self.converts(parser, id, position, end));
        let Some(start) = start else {
            return end.map(|end| (end, None));
        };
        let found = parser.finish_rule(rule, start, end);
        // A rule that recovers goes on from where a parse that recovers tells
        // it to, when it fails.
        if found.is_some() || !plan.recovers {
            return found;
        }
        recovering(id, || parser.recover(rule, start, again::<R, W>))
    }

    /// Returns whether the type the rule numbered `id` converts to, if any,
    /// reads its text from `position` to `end`.
    pub(super) fn converts<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        id: usize,
        position: usize,
        end: usize,
    ) -> bool {
        let Some(ty) = &self.plan_of(id).converts_to else {
            return true;
        };
        accepts(parser, &self.rules[id], ty, position, end)
    }

    /// Matches `part` of the rule numbered `rule` at `position`, as the code
    /// of a rule that takes each way as it comes does.
    fn plain<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        position: usize,
    ) -> Option<usize> {
        match &part.kind {
            Kind::Sequence(steps) => {
                let mut position = position;
                for step in steps {
                    position = self.plain(parser, rule, step, position)?;
                }
                Some(position)
            }
            Kind::Choice(alternatives) => {
                self.choice(parser, rule, part, alternatives, position)
            }
            Kind::Optional(sequence) => {
                if let Some(set) = &sequence.set {
                    let single = single(parser, sequence, set, position);
                    return Some(single.unwrap_or(position));
                }
                let mark = parser.mark();
                if let Some(end) =
                    self.branch(parser, rule, sequence, position)
                {
                    return Some(end);
                }
                parser.truncate(mark);
                Some(position)
            }
            Kind::Repeat(sequence) => {
                if let Some(set) = set_parts(sequence) {
                    let expected = expected(&set.expected);
                    return Some(parser.span(
                        position,
                        contains(&set.ranges),
                        &expected,
                    ));
                }
                let mut end = position;
                loop {
                    let mark = parser.mark();
                    let Some(next) = self.branch(parser, rule, sequence, end)
                    else {
                        parser.truncate(mark);
                        break Some(end);
                    };
                    end = next;
                }
            }
            Kind::Times(times, primary) => {
                let mut position = position;
                for _ in 0..*times {
                    position = self.plain(parser, rule, primary, position)?;
                }
                Some(position)
            }
            Kind::Except { factor, exception } => {
                if let Some((set, excludes)) = sets_apart(part, factor) {
                    return set_except(parser, set, excludes, position);
                }
                let end = self.plain(parser, rule, factor, position)?;
                let mut within = parser.within(end);
                let excluded =
                    self.plain(&mut within, rule, exception, position)
                        == Some(end);
                if excluded {
                    parser.excluded(&self.rules[rule], position, end);
                    None
                } else {
                    Some(end)
                }
            }
            Kind::Rule(id) => {
                let found = self.rule(parser, *id, position);
                parser.matched(found)
            }
            Kind::Text(text) => parser.text(position, text),
            Kind::Range(first, last) => parser.range(position, *first, *last),
            Kind::Empty => Some(position),
            _ => unreachable!("a plan holds no other part"),
        }
    }

    /// Matches the choice `part` among `alternatives`, as the code of a rule
    /// that takes each way as it comes does. A run of alternatives that only
    /// match one character each is read as one set.
    fn choice<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        alternatives: &'static [Part],
        position: usize,
    ) -> Option<usize> {
        let branches = branches(alternatives);
        let take = |parser: &mut Parser<'_, R, W>,
                    branch: &Branch<'static>| {
            match branch {
                Branch::Part(alternative) => {
                    self.branch(parser, rule, alternative, position)
                }
                Branch::Sets(sets) => {
                    let expected: Vec<Expected> = sets
                        .iter()
                        .flat_map(|set| expected(&set.expected))
                        .collect();
                    parser.set(
                        position,
                        |c| sets.iter().any(|set| within(&set.ranges, c)),
                        &expected,
                    )
                }
            }
        };
        if let [branch] = branches.as_slice() {
            return take(parser, branch);
        }
        if self.plan_of(rule).first_match {
            let mark = parser.mark();
            for branch in &branches {
                if let Some(end) = take(parser, branch) {
                    return Some(end);
                }
                parser.truncate(mark);
            }
            return None;
        }
        if part.start_apart {
            let mut found = None;
            for branch in &branches {
                if R || found.is_none() {
                    let start = parser.mark();
                    match take(parser, branch) {
                        Some(end) if found.is_none() => found = Some(end),
                        _ => parser.truncate(start),
                    }
                }
            }
            return found;
        }
        let mark = parser.mark();
        let mut longest: Option<usize> = None;
        for branch in &branches {
            let start = parser.mark();
            match take(parser, branch) {
                Some(end) if longest.is_none_or(|most| end > most) => {
                    parser.discard(mark, start);
                    longest = Some(end);
                }
                _ => parser.truncate(start),
            }
        }
        longest
    }

    /// Matches `part` as a branch. A parse that only matches skips it where
    /// the next character is none of those the part can start with.
    fn branch<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        rule: usize,
        part: &'static Part,
        position: usize,
    ) -> Option<usize> {
        if skips::<R, W>(parser, part, false, position) {
            return None;
        }
        self.plain(parser, rule, part, position)
    }
}
