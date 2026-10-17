//! The parser and the steps that the function of each rule is made of.

mod ends;
mod exact;
mod recovery;
mod runs;

use std::{ops::Range, str::FromStr};

use rustc_hash::FxHashMap;

use super::{
    Expected, Match, Rule,
    nodes::{Nodes, Width},
    tree::Tree,
};
pub use ends::Ends;
use exact::RememberedEnds;
use recovery::Candidate;
pub(crate) use recovery::{Recoveries, recovering};

use crate::error::Error;

/// Matches the rules of a grammar against an input and builds the tree of the
/// match. When `REPORTS` is true, it also notes what could have been read at
/// each failure, to tell why the input does not match. The positions and
/// indices of its nodes are of width `W`.
#[derive(Debug)]
pub struct Parser<'i, const REPORTS: bool, W> {
    input: &'i str,
    /// The nodes built, and those matched inside the rules being matched.
    nodes: Nodes<W>,
    /// Whether there came to be more nodes than `W` counts, so the parser
    /// stopped building them.
    overflowed: bool,
    /// What each rule that remembers its matches matched at each position. The
    /// keys are small numbers an input cannot pick freely, so a fast hasher
    /// that does not resist collisions suits them.
    remembered: FxHashMap<(usize, usize), Remembered>,
    /// The same for the ends of each rule that tries every way through it.
    remembered_ends: FxHashMap<(usize, usize), RememberedEnds>,
    /// The furthest position where something failed to match.
    furthest: usize,
    /// What could have been read at the furthest position.
    expected: Vec<Expected>,
    /// Where the text ends that a rule matched at the furthest position, when
    /// its type did not read it.
    rejected: Option<usize>,
    /// Where each rule that recovers goes on from where its match failed.
    recoveries: Recoveries,
    /// The matches of rules that recover that failed partway.
    candidates: Vec<Candidate>,
    /// Whether the parser is looking for where a rule goes on from, so no rule
    /// recovers.
    scanning: bool,
    /// How many matches of recursive rules are inside one another here.
    depth: usize,
    /// How many matches of recursive rules the parser follows inside one
    /// another.
    limit: usize,
    /// Where the matches of recursive rules first nested more deeply than
    /// `limit`, if they did.
    too_deep: Option<usize>,
    /// The run each place in the grammar that reads a run up to a match of a
    /// middle last read whole, from where to where, for a parse that notes why
    /// it fails.
    runs: FxHashMap<usize, (usize, usize)>,
}

/// What a rule matched at a position, and whether the rule was noted as
/// expected there.
#[derive(Debug, Clone, Copy)]
struct Remembered {
    found: Match,
    noted: bool,
}

/// Where a rule started to match. Finishing the rule needs it.
#[derive(Debug, Clone, Copy)]
pub struct RuleStart {
    position: usize,
    /// How many nodes were matched before the rule.
    mark: usize,
    /// The furthest position where something failed before the rule.
    furthest: usize,
    /// How much was noted as expected there before the rule.
    noted: usize,
}

impl<'i, const REPORTS: bool, W: Width> Parser<'i, REPORTS, W> {
    /// Returns a parser of `input` that follows `limit` matches of recursive
    /// rules inside one another.
    pub(super) fn new(input: &'i str, limit: usize) -> Self {
        Self {
            input,
            nodes: Nodes::new(),
            overflowed: false,
            remembered: FxHashMap::default(),
            remembered_ends: FxHashMap::default(),
            furthest: 0,
            rejected: None,
            expected: Vec::new(),
            recoveries: Recoveries::default(),
            candidates: Vec::new(),
            scanning: false,
            depth: 0,
            limit,
            too_deep: None,
            runs: FxHashMap::default(),
        }
    }

    pub(super) fn error(self) -> Error {
        match self.too_deep {
            Some(offset) => Error::too_deep(self.input, offset),
            None => Error::new(
                self.input,
                self.furthest,
                self.expected,
                self.rejected,
            ),
        }
    }

    /// Returns the length of the input in bytes.
    #[must_use]
    pub fn length(&self) -> usize {
        self.input.len()
    }

    /// Returns whether there came to be more nodes than the parser's width
    /// counts, so that a wider parser has to build them.
    pub(super) fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Returns whether matches nested too deeply, so that no match counts.
    pub(super) fn went_too_deep(&self) -> bool {
        self.too_deep.is_some()
    }

    /// Starts a match of a recursive rule at `position`, unless the matches
    /// nest too deeply there or did so before. Each start that goes through
    /// needs a [`leave`](Self::leave).
    pub fn enter(&mut self, position: usize) -> bool {
        if self.too_deep.is_some() {
            return false;
        }
        if self.depth == self.limit {
            self.too_deep = Some(position);
            return false;
        }
        self.depth += 1;
        true
    }

    /// Finishes a match of a recursive rule.
    pub fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Returns the tree of the nodes built, from the node `root`.
    pub(super) fn tree(self, root: usize) -> Tree {
        Tree::new(self.input, W::store(self.nodes), root)
    }

    /// Notes that `expected` could have been read at `position`.
    pub(super) fn note(&mut self, position: usize, expected: Expected) {
        self.fail(position, &[expected]);
    }

    /// Returns the character at `position`, if the input goes on there.
    #[must_use]
    pub fn character(&self, position: usize) -> Option<char> {
        self.input[position..].chars().next()
    }

    /// Returns where `text` ends when the input has it at `position`.
    pub fn text(
        &mut self,
        position: usize,
        text: &'static str,
    ) -> Option<usize> {
        if self.input[position..].starts_with(text) {
            return Some(position + text.len());
        }
        self.note(position, Expected::Text(text));
        None
    }

    /// Returns where the character at `position` ends when it is from `first`
    /// to `last`.
    pub fn range(
        &mut self,
        position: usize,
        first: char,
        last: char,
    ) -> Option<usize> {
        let next = self.character(position);
        if let Some(c) = next.filter(|c| (first..=last).contains(c)) {
            return Some(position + c.len_utf8());
        }
        self.note(position, Expected::Range(first, last));
        None
    }

    /// Returns where the character at `position` ends when `contains` holds
    /// for it. Otherwise notes each part of the set in `expected`.
    pub fn set(
        &mut self,
        position: usize,
        contains: impl Fn(char) -> bool,
        expected: &[Expected],
    ) -> Option<usize> {
        if let Some(c) = self.character(position).filter(|&c| contains(c)) {
            return Some(position + c.len_utf8());
        }
        self.fail(position, expected);
        None
    }

    /// Returns where the character at `position` ends when `contains` holds
    /// for it and `excludes` does not, as for an exception of single
    /// characters. A character `contains` does not hold for notes each of
    /// `expected`. One the exception takes out notes nothing, since an
    /// exception tells nothing about why the input does not match.
    pub fn set_except(
        &mut self,
        position: usize,
        contains: impl Fn(char) -> bool,
        excludes: impl Fn(char) -> bool,
        expected: &[Expected],
    ) -> Option<usize> {
        match self.character(position) {
            Some(c) if contains(c) => {
                (!excludes(c)).then(|| position + c.len_utf8())
            }
            _ => {
                self.fail(position, expected);
                None
            }
        }
    }

    /// Returns where the longest run of characters from `position` that
    /// `contains` holds for ends, and notes each part of the set in `expected`
    /// there.
    pub fn span(
        &mut self,
        position: usize,
        contains: impl Fn(char) -> bool,
        expected: &[Expected],
    ) -> usize {
        let rest = &self.input[position..];
        let end = position + rest.find(|c| !contains(c)).unwrap_or(rest.len());
        self.fail(end, expected);
        end
    }

    /// Notes each of `expected` as what could have been read where something
    /// failed at `position`. With nothing to note, the failure still counts as
    /// the furthest, as a hidden rule's does.
    fn fail(&mut self, position: usize, expected: &[Expected]) {
        if !REPORTS || position < self.furthest {
            return;
        }
        if position > self.furthest {
            self.furthest = position;
            self.rejected = None;
            self.expected.clear();
        }
        self.expected.extend_from_slice(expected);
    }

    /// Returns whether `T` reads the text `rule` matched from `position` to
    /// `end`. When it does not, notes the rule there as expected, with that
    /// text as what was found.
    pub fn accepts<T: FromStr>(
        &mut self,
        rule: &Rule,
        position: usize,
        end: usize,
    ) -> bool {
        if self.input[position..end].parse::<T>().is_ok() {
            return true;
        }
        self.reject(rule, position, end);
        false
    }

    /// Notes `rule` as expected at `position`, with the text from there to
    /// `end` as what was found, since an exception took that match away.
    pub fn excluded(&mut self, rule: &Rule, position: usize, end: usize) {
        self.reject(rule, position, end);
    }

    /// Notes `rule` as expected at `position`, with the text from there to
    /// `end` as what was found, since something took its match there away.
    fn reject(&mut self, rule: &Rule, position: usize, end: usize) {
        // A failure inside text a rule matched in full tells how far the match
        // could go, not why the input does not match, so what took the match
        // away tells it instead.
        if REPORTS && self.furthest <= end {
            if self.furthest > position {
                self.furthest = position;
                self.expected.clear();
                self.rejected = None;
            }
            self.fail(position, &[Expected::Rule(rule.name)]);
            self.rejected =
                Some(self.rejected.map_or(end, |most| most.max(end)));
        }
    }

    /// Returns how many nodes have been matched, to go back to with
    /// [`Parser::truncate`] when what comes next does not match.
    #[must_use]
    pub fn mark(&self) -> usize {
        self.nodes.mark()
    }

    /// Forgets the nodes matched since `mark`.
    pub fn truncate(&mut self, mark: usize) {
        self.nodes.truncate(mark);
    }

    /// Forgets the nodes from `mark` to `start` of a branch that a longer one
    /// replaced.
    pub fn discard(&mut self, mark: usize, start: usize) {
        self.nodes.discard(mark, start);
    }

    /// Adds the node of the rule's match `found` to the nodes matched so far,
    /// and returns where the match ends.
    pub fn matched(&mut self, found: Match) -> Option<usize> {
        let (end, node) = found?;
        self.add_node(node);
        Some(end)
    }

    /// Adds the node a rule built to the nodes matched so far. A hidden rule
    /// builds none.
    pub fn add_node(&mut self, node: Option<usize>) {
        if let Some(node) = node {
            self.nodes.add(node);
        }
    }

    /// Returns what `rule` matched at `position`, when it remembers that. When
    /// the rule was noted as expected there, it is noted again, as matching it
    /// again would.
    pub fn recall(&mut self, rule: &Rule, position: usize) -> Option<Match> {
        let remembered = *self.remembered.get(&(rule.number, position))?;
        if remembered.noted {
            self.note(position, Expected::Rule(rule.name));
        }
        Some(remembered.found)
    }

    /// Starts to match a rule at `position`.
    #[must_use]
    pub fn start_rule(&self, position: usize) -> RuleStart {
        RuleStart {
            position,
            mark: self.nodes.mark(),
            furthest: self.furthest,
            noted: self.expected.len(),
        }
    }

    /// Finishes `rule` and returns its match from `start` up to `end`, or none
    /// when `end` is `None`. A rule that does not match forgets the nodes it
    /// matched, so no caller finds them among its own.
    pub fn finish_rule(
        &mut self,
        rule: &Rule,
        start: RuleStart,
        end: Option<usize>,
    ) -> Match {
        if end.is_none() {
            self.nodes.truncate(start.mark);
        }
        let found = end.map(|end| {
            (end, self.node(rule, start.position..end, start.mark))
        });
        let noted = self.note_rule(rule, start);
        if rule.remembers {
            let remembered = Remembered { found, noted };
            self.remembered
                .insert((rule.number, start.position), remembered);
        }
        found
    }

    /// Replaces what `rule` noted where it starts with the rule itself, and
    /// returns whether it did. A start rule and most hidden rules leave what
    /// they noted as it is, since their name would say less.
    fn note_rule(&mut self, rule: &Rule, start: RuleStart) -> bool {
        if !REPORTS {
            return false;
        }
        // What a quiet rule could not read never tells why the input does not
        // match, even where the rule ends.
        if rule.quiet {
            if self.furthest == start.furthest {
                self.expected.truncate(start.noted);
            } else {
                self.expected.clear();
            }
            return false;
        }
        let noted_before = if start.furthest == start.position {
            start.noted
        } else {
            0
        };
        if rule.start
            || self.furthest != start.position
            || self.expected.len() <= noted_before
        {
            return false;
        }
        // A hidden rule leaves what it could not read for the rule around it
        // to tell, such as a closing bracket, unless that holds ranges of
        // characters, which its name tells better.
        let ranges = self.expected[noted_before..].iter().any(|expected| {
            matches!(expected, Expected::Range(first, last) if first != last)
        });
        if rule.hidden && !ranges {
            return false;
        }
        self.expected.truncate(noted_before);
        self.expected.push(Expected::Rule(rule.name));
        true
    }

    /// Returns the node of `rule` that matched `range`, with the nodes matched
    /// since `mark` as its children. A rule that builds no node drops those
    /// nodes.
    fn node(
        &mut self,
        rule: &Rule,
        range: Range<usize>,
        mark: usize,
    ) -> Option<usize> {
        if !rule.builds || self.overflowed {
            self.nodes.truncate(mark);
            return None;
        }
        let number = u32::try_from(rule.number)
            .expect("a grammar has fewer rules than `u32` holds");
        let node = self.nodes.node(number, rule.inline, &range, mark);
        // A parser whose nodes outgrow its width only goes on matching, and a
        // wider one builds the tree.
        if node.is_none() {
            self.overflowed = true;
            self.nodes.truncate(mark);
        }
        node
    }

    /// Returns a parser of the input up to `end`, for an exception. An
    /// exception only reads what its factor matched, and its failures tell
    /// nothing about why the input does not match.
    #[must_use]
    pub fn within(&self, end: usize) -> Parser<'i, false, W> {
        Parser::new(&self.input[..end], self.limit)
    }
}
