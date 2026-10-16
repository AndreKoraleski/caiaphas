//! The functions of a rule that tries every way through it work with the ends
//! of its expressions.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{Parser, RuleStart};
use crate::__private::{Expected, Rule};

/// Where the matches of an expression at a position can end, each once, in the
/// order a parser prefers them.
#[derive(Debug, Clone)]
pub struct Ends(Held);

/// How [`Ends`] holds its ends. Most expressions have few, so those are held
/// without allocating.
#[derive(Debug, Clone)]
enum Held {
    Few {
        count: usize,
        ends: [usize; FEW_ENDS],
    },
    Many {
        ends: Vec<usize>,
        /// The same ends once there are more than [`LISTED_ENDS`], to tell
        /// quickly whether one is there.
        seen: Option<FxHashSet<usize>>,
    },
}

/// How many ends [`Ends`] holds without allocating.
const FEW_ENDS: usize = 4;

/// How many ends [`Ends`] looks through one by one.
const LISTED_ENDS: usize = 16;

impl Default for Ends {
    fn default() -> Self {
        Self(Held::Few {
            count: 0,
            ends: [0; FEW_ENDS],
        })
    }
}

impl Ends {
    /// Returns no ends, as of an expression that does not match.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn one(end: usize) -> Self {
        let mut ends = Self::none();
        ends.add(end);
        ends
    }

    /// Returns `end` alone, if there is one.
    pub fn of(end: Option<usize>) -> Self {
        end.map_or_else(Self::none, Self::one)
    }

    fn as_slice(&self) -> &[usize] {
        match &self.0 {
            Held::Few { count, ends } => &ends[..*count],
            Held::Many { ends, .. } => ends,
        }
    }

    pub fn contains(&self, end: usize) -> bool {
        match &self.0 {
            Held::Many {
                seen: Some(seen), ..
            } => seen.contains(&end),
            _ => self.as_slice().contains(&end),
        }
    }

    /// Returns the end a parser prefers, if any.
    pub fn first(&self) -> Option<usize> {
        self.as_slice().first().copied()
    }

    /// Returns the ends in order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.as_slice().iter().copied()
    }

    /// Adds `end` after the others, unless it is there already.
    pub fn add(&mut self, end: usize) {
        if self.contains(end) {
            return;
        }
        match &mut self.0 {
            Held::Few { count, ends } if *count < FEW_ENDS => {
                ends[*count] = end;
                *count += 1;
            }
            Held::Few { ends, .. } => {
                let mut many = Vec::with_capacity(FEW_ENDS * 2);
                many.extend_from_slice(ends);
                many.push(end);
                self.0 = Held::Many {
                    ends: many,
                    seen: None,
                };
            }
            Held::Many { ends, seen } => {
                ends.push(end);
                match seen {
                    Some(seen) => {
                        seen.insert(end);
                    }
                    None if ends.len() > LISTED_ENDS => {
                        *seen = Some(ends.iter().copied().collect());
                    }
                    None => {}
                }
            }
        }
    }

    /// Adds each of `other` after these, in its order.
    pub fn extend(&mut self, other: Self) {
        for &end in other.as_slice() {
            self.add(end);
        }
    }

    /// Keeps only the ends `keep` holds for.
    pub fn retain(&mut self, mut keep: impl FnMut(usize) -> bool) {
        let kept = Self::none();
        let kept = self.iter().filter(|&end| keep(end)).fold(
            kept,
            |mut kept, end| {
                kept.add(end);
                kept
            },
        );
        *self = kept;
    }

    /// Returns the ends with the end of the most text first.
    pub fn longest_first(mut self) -> Self {
        let ends = match &mut self.0 {
            Held::Few { count, ends } => &mut ends[..*count],
            Held::Many { ends, .. } => ends,
        };
        ends.sort_unstable_by(|left, right| right.cmp(left));
        self
    }

    /// Returns the only end of an expression that has at most one, and that
    /// the rest of a match goes on from.
    pub fn only(&self) -> usize {
        let ends = self.as_slice();
        debug_assert_eq!(ends.len(), 1, "the expression has one end");
        ends[0]
    }
}

/// What tells where a step of a sequence can end from where it starts.
pub type Step<'s, 'i, const REPORTS: bool> =
    &'s dyn Fn(&mut Parser<'i, REPORTS>, usize) -> Ends;

/// Where a rule's matches at a position can end, and whether the rule was
/// noted as expected there.
#[derive(Debug, Clone)]
pub(super) struct RememberedEnds {
    ends: Ends,
    noted: bool,
}

impl<'i, const REPORTS: bool> Parser<'i, REPORTS> {
    /// Returns every place where the characters from `position` on that
    /// `contains` holds for can end, the furthest first, and notes each of
    /// `expected` at the furthest.
    pub fn span_ends(
        &mut self,
        position: usize,
        contains: impl Fn(char) -> bool,
        expected: &[Expected],
    ) -> Ends {
        let end = self.span(position, contains, expected);
        let mut ends = Ends::one(end);
        for (offset, _) in self.input[position..end].char_indices().rev() {
            ends.add(position + offset);
        }
        ends
    }

    /// Returns `part` as it is. A closure that tells where a part of an
    /// expression can end goes through it to take this parser.
    pub fn part<F: Fn(&mut Self, usize) -> Ends>(&self, part: F) -> F {
        part
    }

    /// Returns where `step` can end after each of `ends`, in the order of
    /// `ends` and then of what `step` prefers.
    pub fn then(
        &mut self,
        ends: Ends,
        mut step: impl FnMut(&mut Self, usize) -> Ends,
    ) -> Ends {
        let mut after = Ends::none();
        for &end in ends.as_slice() {
            after.extend(step(self, end));
        }
        after
    }

    /// Returns where `step` can end after each of `ends`, taken `count` times
    /// one after another.
    pub fn times(
        &mut self,
        mut ends: Ends,
        count: u32,
        mut step: impl FnMut(&mut Self, usize) -> Ends,
    ) -> Ends {
        for _ in 0..count {
            ends = self.then(ends, &mut step);
        }
        ends
    }

    /// Returns where `round`, taken any number of times one after another from
    /// `position`, can end. Each way that takes one more round comes before
    /// stopping, and the analysis makes sure a round never matches nothing.
    pub fn rounds(
        &mut self,
        position: usize,
        mut round: impl FnMut(&mut Self, usize) -> Ends,
    ) -> Ends {
        let mut ends = Ends::none();
        let mut reached = Ends::one(position);
        // Each round still being followed, with where it starts, where it can
        // end, and how many of those have been followed.
        let mut followed = vec![(position, round(self, position), 0)];
        while let Some((start, nexts, taken)) = followed.last_mut() {
            let (start, next) =
                (*start, nexts.as_slice().get(*taken).copied());
            *taken += 1;
            match next {
                Some(next) if !reached.contains(next) => {
                    reached.add(next);
                    let after = round(self, next);
                    followed.push((next, after, 0));
                }
                Some(_) => {}
                None => {
                    ends.add(start);
                    followed.pop();
                }
            }
        }
        ends
    }

    /// Returns where each of `steps`, taken one after another from `position`
    /// to `end`, ends in the way a parser prefers, where each step takes the
    /// first end it prefers that the steps after it can go on from to `end`.
    pub fn split(
        &mut self,
        steps: &[Step<'_, 'i, REPORTS>],
        position: usize,
        end: usize,
    ) -> Vec<usize> {
        let mut split = Vec::with_capacity(steps.len());
        let mut failed = FxHashSet::default();
        let found = self.leads(steps, position, end, &mut failed, &mut split);
        debug_assert!(found, "the steps lead to `end`");
        split
    }

    /// Returns whether `steps` can go on from `start` to `end`, adding where
    /// each ends in the way a parser prefers to `split`. `failed` holds how
    /// many steps were left, and where, for each try that could not.
    fn leads(
        &mut self,
        steps: &[Step<'_, 'i, REPORTS>],
        start: usize,
        end: usize,
        failed: &mut FxHashSet<(usize, usize)>,
        split: &mut Vec<usize>,
    ) -> bool {
        let Some((step, rest)) = steps.split_first() else {
            return start == end;
        };
        if failed.contains(&(steps.len(), start)) {
            return false;
        }
        let candidates = step(self, start);
        if rest.is_empty() {
            let found = candidates.contains(end);
            if found {
                split.push(end);
            }
            return found;
        }
        for candidate in
            candidates.iter().filter(|&candidate| candidate <= end)
        {
            split.push(candidate);
            if self.leads(rest, candidate, end, failed, split) {
                return true;
            }
            split.pop();
        }
        failed.insert((steps.len(), start));
        false
    }

    /// Returns where each round ends in the way through rounds of `round` from
    /// `position` to `end` that a parser prefers, where each round takes the
    /// first end it prefers that more rounds can go on from to `end`.
    pub fn rounds_to(
        &mut self,
        position: usize,
        end: usize,
        mut round: impl FnMut(&mut Self, usize) -> Ends,
    ) -> Vec<usize> {
        // Where a round from each place a round can start can end, up to
        // `end`.
        let mut nexts: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
        let mut pending = vec![position];
        while let Some(start) = pending.pop() {
            if start == end || nexts.contains_key(&start) {
                continue;
            }
            let ends: Vec<usize> = round(self, start)
                .iter()
                .filter(|&next| next <= end)
                .collect();
            pending.extend(&ends);
            nexts.insert(start, ends);
        }
        // Which of those places lead to `end`, from the furthest back.
        let mut starts: Vec<usize> = nexts.keys().copied().collect();
        starts.sort_unstable_by(|left, right| right.cmp(left));
        let mut leading = FxHashSet::default();
        leading.insert(end);
        for start in starts {
            if nexts[&start].iter().any(|next| leading.contains(next)) {
                leading.insert(start);
            }
        }
        let mut rounds = Vec::new();
        let mut start = position;
        while start != end {
            start = *nexts[&start]
                .iter()
                .find(|next| leading.contains(next))
                .expect("the ends of the rounds lead to `end`");
            rounds.push(start);
        }
        rounds
    }

    /// Returns the first of `candidates` that `leads` holds for. The ends of
    /// what they are part of make sure there is one.
    pub fn choose(
        &mut self,
        candidates: Ends,
        mut leads: impl FnMut(&mut Self, usize) -> bool,
    ) -> usize {
        candidates
            .iter()
            .find(|&candidate| leads(self, candidate))
            .expect("one of the candidates leads to the end")
    }

    /// Returns where `rule`'s matches at `position` can end, when it remembers
    /// that, noting it again as [`Parser::recall`] does.
    pub fn recall_ends(
        &mut self,
        rule: &Rule,
        position: usize,
    ) -> Option<Ends> {
        let remembered = self.remembered_ends.get(&(rule.number, position))?;
        let (ends, noted) = (remembered.ends.clone(), remembered.noted);
        if noted {
            self.note(position, Expected::Rule(rule.name));
        }
        Some(ends)
    }

    /// Finishes `rule`, whose matches from `start` can end at `ends`,
    /// remembers them, and returns them.
    pub fn finish_ends(
        &mut self,
        rule: &Rule,
        start: RuleStart,
        ends: Ends,
    ) -> Ends {
        let noted = self.note_rule(rule, start);
        let remembered = RememberedEnds {
            ends: ends.clone(),
            noted,
        };
        self.remembered_ends
            .insert((rule.number, start.position), remembered);
        ends
    }

    /// Returns the node of `rule` built from `start` up to `end`, whose
    /// children are the nodes built since.
    pub fn built(
        &mut self,
        rule: &Rule,
        start: RuleStart,
        end: usize,
    ) -> Option<usize> {
        self.node(rule, start.position..end, start.mark)
    }
}
