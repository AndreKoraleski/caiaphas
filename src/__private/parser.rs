//! The parser and the steps that the function of each rule is made of.

mod ends;

use std::ops::Range;

use rustc_hash::FxHashMap;

use super::{
    Expected, Match, Rule,
    tree::{NodeData, Tree},
};
pub use ends::Ends;
use ends::RememberedEnds;

use crate::error::Error;

/// Matches the rules of a grammar against an input and builds the tree of the
/// match. When `REPORTS` is true, it also notes what could have been read at
/// each failure, to tell why the input does not match.
#[derive(Debug)]
pub struct Parser<'i, const REPORTS: bool> {
    input: &'i str,
    nodes: Vec<NodeData>,
    /// The children of every node, as in [`Tree`].
    children: Vec<usize>,
    /// The nodes matched inside the rules being matched, those of each rule
    /// after those of the rules around it.
    matched: Vec<usize>,
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
}

/// What a rule matched at a position, and whether the rule was noted as
/// expected there.
#[derive(Debug, Clone, Copy)]
struct Remembered {
    found: Match,
    noted: bool,
}

/// Where a rule started to match, which finishing it needs.
#[derive(Debug, Clone, Copy)]
pub struct RuleStart {
    position: usize,
    /// How many nodes were matched before the rule.
    mark: usize,
    /// How much was noted as expected at the furthest position before the
    /// rule.
    noted_before: usize,
}

impl<'i, const REPORTS: bool> Parser<'i, REPORTS> {
    pub(super) fn new(input: &'i str) -> Self {
        Self {
            input,
            nodes: Vec::new(),
            children: Vec::new(),
            matched: Vec::new(),
            remembered: FxHashMap::default(),
            remembered_ends: FxHashMap::default(),
            furthest: 0,
            expected: Vec::new(),
        }
    }

    pub(super) fn error(self) -> Error {
        Error::new(self.input, self.furthest, self.expected)
    }

    /// Returns the tree of the nodes built, from the node `root`.
    pub(super) fn tree(self, root: usize) -> Tree {
        Tree::new(self.input, self.nodes, self.children, root)
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
    /// for it. Otherwise notes each of `expected`, which the set is made of.
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

    /// Returns where the characters from `position` on that `contains` holds
    /// for end, and notes each of `expected`, which the set is made of, there.
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
            self.expected.clear();
        }
        self.expected.extend_from_slice(expected);
    }

    /// Returns how many nodes have been matched, to go back to with
    /// [`Parser::truncate`] when what comes next does not match.
    #[must_use]
    pub fn mark(&self) -> usize {
        self.matched.len()
    }

    /// Forgets the nodes matched since `mark`.
    pub fn truncate(&mut self, mark: usize) {
        self.matched.truncate(mark);
    }

    /// Forgets the nodes matched from `mark` to `start`, which a branch
    /// matched before a longer one.
    pub fn discard(&mut self, mark: usize, start: usize) {
        self.matched.drain(mark..start);
    }

    /// Adds the node of `found`, which a rule matched, to the nodes matched so
    /// far, and returns where the match ends.
    pub fn matched(&mut self, found: Match) -> Option<usize> {
        let (end, node) = found?;
        self.add_node(node);
        Some(end)
    }

    /// Adds `node`, which a rule built, to the nodes matched so far. A hidden
    /// rule has none.
    pub fn add_node(&mut self, node: Option<usize>) {
        self.matched.extend(node);
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
        let noted_before = if REPORTS && self.furthest == position {
            self.expected.len()
        } else {
            0
        };
        RuleStart {
            position,
            mark: self.matched.len(),
            noted_before,
        }
    }

    /// Finishes `rule`, which matched from `start` up to `end`, or not at all,
    /// and returns its match.
    pub fn finish_rule(
        &mut self,
        rule: &Rule,
        start: RuleStart,
        end: Option<usize>,
    ) -> Match {
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

    /// Tells what `rule`, started at `start`, could not read where it starts
    /// as the rule itself, unless it is hidden. Returns whether it did.
    fn note_rule(&mut self, rule: &Rule, start: RuleStart) -> bool {
        if !REPORTS
            || self.furthest != start.position
            || self.expected.len() <= start.noted_before
        {
            return false;
        }
        // A hidden rule leaves what it could not read for the rule around it
        // to tell, unless it is quiet.
        if rule.quiet {
            self.expected.truncate(start.noted_before);
        }
        if rule.hidden {
            return false;
        }
        self.expected.truncate(start.noted_before);
        self.expected.push(Expected::Rule(rule.name));
        true
    }

    /// Returns the node of `rule` that matched `range`, whose children are the
    /// nodes matched since `mark`. A hidden rule has no node, so it drops
    /// those nodes.
    fn node(
        &mut self,
        rule: &Rule,
        range: Range<usize>,
        mark: usize,
    ) -> Option<usize> {
        if rule.hidden {
            self.matched.truncate(mark);
            return None;
        }
        // A child of an inline rule stands where its node would.
        let first = self.children.len();
        for child in self.matched.drain(mark..) {
            let data = &self.nodes[child];
            if data.inline {
                self.children.extend_from_within(data.children.clone());
            } else {
                self.children.push(child);
            }
        }
        self.nodes.push(NodeData {
            rule: u32::try_from(rule.number)
                .expect("a grammar has fewer rules than `u32` holds"),
            inline: rule.inline,
            start: range.start,
            end: range.end,
            children: first..self.children.len(),
        });
        Some(self.nodes.len() - 1)
    }

    /// Returns a parser of the input up to `end`, for an exception. An
    /// exception only reads what its factor matched, and its failures tell
    /// nothing about why the input does not match.
    #[must_use]
    pub fn within(&self, end: usize) -> Parser<'i, false> {
        Parser::new(&self.input[..end])
    }
}
