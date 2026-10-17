//! Recovery lets a parse go on past the places where its input does not match,
//! to tell every error instead of only the first.
//!
//! A rule that recovers and fails partway, where its own attempt reached the
//! error, is skipped ahead: the parser looks for its next match from the error
//! on and takes that instead, leaving out the text between. Each parse finds
//! one more error, and the next parse recovers from it, until the input
//! matches or no rule can recover. The places a rule went on from carry over
//! from one parse to the next, so each recovers the same way every time.

use std::mem;

use rustc_hash::FxHashMap;

use super::{
    Ends, Parser, Remembered, RuleStart, Width, exact::RememberedEnds,
};
use crate::{
    __private::{Expected, Match, Rule, Start, Tree, matched},
    error::Error,
};

/// How many errors a parse that recovers tells at most.
const ERRORS: usize = 100;

/// How a rule that recovers goes on where its match from a place failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recovery {
    /// It looks for its next match from this place on.
    From(usize),
    /// It takes its match from this place.
    At(usize),
    /// No match comes later, so it fails as it did.
    Failed,
}

/// The places each rule that recovers goes on from, by the rule's number and
/// where its match failed.
pub(crate) type Recoveries = FxHashMap<(usize, usize), Recovery>;

/// A match of a rule that recovers, which failed partway.
#[derive(Debug, Clone, Copy)]
pub(super) struct Candidate {
    rule: usize,
    position: usize,
    /// The furthest failure before the match started.
    before: usize,
    /// The furthest failure once the match failed.
    reached: usize,
}

impl<const REPORTS: bool, W: Width> Parser<'_, REPORTS, W> {
    /// Returns what `rule` matches where its match from `start` failed, for a
    /// rule that recovers: its match from the place it goes on from, when
    /// there is one. `again` matches the rule.
    pub fn recover(
        &mut self,
        rule: &Rule,
        start: RuleStart,
        again: fn(&mut Self, usize) -> Match,
    ) -> Match {
        let at = self
            .going_on(rule, start, |parser, at| again(parser, at).is_some())?;
        let found = again(self, at);
        if rule.remembers {
            let remembered = Remembered {
                found,
                noted: false,
            };
            self.remembered
                .insert((rule.number, start.position), remembered);
        }
        found
    }

    /// Returns what [`recover`](Self::recover) does, for the ends of a rule
    /// that tries every way. `again` tells where the rule's matches end.
    pub fn recover_ends(
        &mut self,
        rule: &Rule,
        start: RuleStart,
        again: fn(&mut Self, usize) -> Ends,
    ) -> Ends {
        let Some(at) = self.going_on(rule, start, |parser, at| {
            !again(parser, at).as_slice().is_empty()
        }) else {
            return Ends::none();
        };
        let ends = again(self, at);
        let remembered = RememberedEnds {
            ends: ends.clone(),
            noted: false,
        };
        self.remembered_ends
            .insert((rule.number, start.position), remembered);
        ends
    }

    /// Returns where the node of `rule` matched from `position` starts: the
    /// place it went on from, when it recovered there.
    #[must_use]
    pub fn recovered(&self, rule: &Rule, position: usize) -> usize {
        match self.recoveries.get(&(rule.number, position)) {
            Some(&Recovery::At(at)) => at,
            _ => position,
        }
    }

    /// Returns where `rule` goes on from after it failed from `start`, when it
    /// does. Looking for that place matches the rule with `matches` at each
    /// place from the error on, without recovering again. Where nothing tells
    /// the rule to go on, a failure that reached further than any before it is
    /// noted as one the rule could recover from.
    fn going_on(
        &mut self,
        rule: &Rule,
        start: RuleStart,
        matches: impl Fn(&mut Self, usize) -> bool,
    ) -> Option<usize> {
        if self.scanning {
            return None;
        }
        let key = (rule.number, start.position);
        let at = match self.recoveries.get(&key).copied() {
            Some(Recovery::At(at)) => at,
            Some(Recovery::Failed) => return None,
            Some(Recovery::From(from)) => {
                let failure = (
                    self.furthest,
                    mem::take(&mut self.expected),
                    self.rejected,
                );
                self.scanning = true;
                let mut found = None;
                for (offset, _) in self.input[from..].char_indices() {
                    // A match that fails partway is an error of its own, so
                    // the rule goes on from there to tell it, instead of
                    // skipping past it.
                    let at = from + offset;
                    self.furthest = at;
                    if matches(self, at) || (REPORTS && self.furthest > at) {
                        found = Some(at);
                        break;
                    }
                }
                self.scanning = false;
                let Some(at) = found else {
                    (self.furthest, self.expected, self.rejected) = failure;
                    self.recoveries.insert(key, Recovery::Failed);
                    return None;
                };
                self.recoveries.insert(key, Recovery::At(at));
                at
            }
            None => {
                if REPORTS
                    && self.furthest > start.position
                    && self.furthest > start.furthest
                {
                    self.candidates.push(Candidate {
                        rule: rule.number,
                        position: start.position,
                        before: start.furthest,
                        reached: self.furthest,
                    });
                }
                return None;
            }
        };
        // What was noted up to here told the error just recovered from.
        self.furthest = 0;
        self.expected.clear();
        self.rejected = None;
        Some(at)
    }
}

/// Returns the tree of `input` and every error in it, the tree only when the
/// input matches once each error is recovered from, following `depth` matches
/// of recursive rules inside one another. `S` is the start rule.
pub(crate) fn recovering<S: Start>(
    input: &str,
    depth: usize,
) -> (Option<Tree>, Vec<Error>) {
    if let Some(tree) = matched::<S>(input, depth, &Recoveries::default()) {
        return (Some(tree), Vec::new());
    }
    let mut recoveries = Recoveries::default();
    let mut errors: Vec<Error> = Vec::new();
    loop {
        // A parse that notes why it fails finds where each rule goes on, since
        // it sees how far each match gets. A parse that only matches then
        // builds the tree from the same places.
        let mut parser =
            Parser::<true, u64>::recovering(input, depth, recoveries);
        let ends = S::explain(&mut parser);
        // An input that nests too deeply cannot be parsed further on either.
        if parser.went_too_deep() {
            errors.push(parser.error());
            return (None, errors);
        }
        if ends.contains(&input.len()) {
            return (matched::<S>(input, depth, &parser.recoveries), errors);
        }
        for end in ends {
            parser.note(end, Expected::End);
        }
        let furthest = parser.furthest;
        let candidate = parser
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.reached == furthest && candidate.before < furthest
            })
            .max_by_key(|candidate| candidate.position)
            .copied();
        recoveries = mem::take(&mut parser.recoveries);
        // An error no further on than the last one would only repeat it.
        if errors.last().is_some_and(|last| furthest <= last.offset()) {
            return (None, errors);
        }
        errors.push(parser.error());
        let Some(candidate) = candidate else {
            return (None, errors);
        };
        let key = (candidate.rule, candidate.position);
        if errors.len() == ERRORS || recoveries.contains_key(&key) {
            return (None, errors);
        }
        recoveries.insert(key, Recovery::From(furthest));
    }
}

impl<'i, const REPORTS: bool, W: Width> Parser<'i, REPORTS, W> {
    /// Hands the parser the places where each rule that recovers goes on.
    pub(crate) fn recover_as(&mut self, recoveries: Recoveries) {
        self.recoveries = recoveries;
    }

    /// Returns a parser of `input` whose rules that recover go on as
    /// `recoveries` tells, following `depth` matches of recursive rules inside
    /// one another.
    fn recovering(
        input: &'i str,
        depth: usize,
        recoveries: Recoveries,
    ) -> Self {
        let mut parser = Self::new(input, depth);
        parser.recoveries = recoveries;
        parser
    }
}
