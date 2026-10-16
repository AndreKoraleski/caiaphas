//! The functions of a rule that tries every way through it work with the ends
//! of its expressions.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{Parser, RuleStart};
use crate::__private::{Expected, Rule};

/// Where the matches of an expression at a position can end, each once, in the
/// order a parser prefers them.
#[derive(Debug, Clone, Default)]
pub struct Ends {
    ends: Vec<usize>,
    /// The same ends once there are more than [`FEW_ENDS`], to tell quickly
    /// whether one is there.
    seen: Option<FxHashSet<usize>>,
}

/// How many ends [`Ends`] looks through one by one.
const FEW_ENDS: usize = 16;

impl Ends {
    /// Returns no ends, as of an expression that does not match.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn one(end: usize) -> Self {
        Self {
            ends: vec![end],
            seen: None,
        }
    }

    /// Returns `end` alone, if there is one.
    pub fn of(end: Option<usize>) -> Self {
        end.map_or_else(Self::none, Self::one)
    }

    pub fn contains(&self, end: usize) -> bool {
        match &self.seen {
            Some(seen) => seen.contains(&end),
            None => self.ends.contains(&end),
        }
    }

    /// Returns the ends in order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.ends.iter().copied()
    }

    /// Adds `end` after the others, unless it is there already.
    pub fn add(&mut self, end: usize) {
        if self.contains(end) {
            return;
        }
        self.ends.push(end);
        match &mut self.seen {
            Some(seen) => {
                seen.insert(end);
            }
            None if self.ends.len() > FEW_ENDS => {
                self.seen = Some(self.ends.iter().copied().collect());
            }
            None => {}
        }
    }

    /// Adds each of `other` after these, in its order.
    pub fn extend(&mut self, other: Self) {
        for end in other.ends {
            self.add(end);
        }
    }

    /// Keeps only the ends `keep` holds for.
    pub fn retain(&mut self, mut keep: impl FnMut(usize) -> bool) {
        self.ends.retain(|&end| keep(end));
        self.seen = (self.ends.len() > FEW_ENDS)
            .then(|| self.ends.iter().copied().collect());
    }

    /// Returns the ends with the end of the most text first.
    pub fn longest_first(mut self) -> Self {
        self.ends.sort_unstable_by(|left, right| right.cmp(left));
        self
    }

    /// Returns the only end of an expression that has at most one, and that
    /// the rest of a match goes on from.
    pub fn only(&self) -> usize {
        debug_assert_eq!(self.ends.len(), 1, "the expression has one end");
        self.ends[0]
    }
}

/// Where a rule's matches at a position can end, and whether the rule was
/// noted as expected there.
#[derive(Debug, Clone)]
pub(super) struct RememberedEnds {
    ends: Ends,
    noted: bool,
}

impl<const REPORTS: bool> Parser<'_, REPORTS> {
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
        for end in ends.ends {
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
            let (start, next) = (*start, nexts.ends.get(*taken).copied());
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
            let mut ends = round(self, start).ends;
            ends.retain(|&next| next <= end);
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
            .ends
            .into_iter()
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
