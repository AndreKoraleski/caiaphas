//! What generated parsers are made of. Generated code writes a function for
//! each rule of its grammar, which matches the rule with a [`Parser`], and
//! turns the tree the parser builds into its own types. Nothing here is meant
//! to be used directly, so it can change in any release.
//!
//! Each choice takes the branch that matches the most text, and the first
//! written of those that tie, unless its rule takes the first match, when it
//! takes the first branch that matches. Optional and repeated sequences take
//! as much as they can. A rule that a parser can try more than once at the
//! same place remembers its match there, so trying every branch never matches
//! it twice.
//!
//! A parse notes nothing about why an input might not match, and skips the
//! branches that cannot start with the next character. Only when the input
//! does not match is it matched again, trying every branch and noting at each
//! failure what could have been read, to say why. The function of each rule is
//! generic over which of the two it does, so the first leaves out the work of
//! the second entirely.

use std::{ops::Range, sync::Arc};

use rustc_hash::FxHashMap;

use crate::{
    error::Error,
    span::{Input, Span},
};

pub use crate::error::Expected;

/// Where a rule's match ends, with its node unless the rule is hidden, or
/// `None` when it does not match.
pub type Match = Option<(usize, Option<usize>)>;

/// What a parser needs to know about a rule besides how to match it.
#[derive(Debug)]
pub struct Rule {
    pub number: usize,
    /// The name of the rule as written. Errors call the rule by it.
    pub name: &'static str,
    /// Whether the rule is left out of the tree and of errors, along with what
    /// it matches.
    pub hidden: bool,
    /// Whether the rule remembers its match at each position.
    pub remembers: bool,
}

/// The function of a rule, which matches the rule at a position.
pub type RuleFunction<const REPORTS: bool> =
    fn(&mut Parser<'_, REPORTS>, usize) -> Match;

/// Returns the tree of `input` matched in full by the rule whose function is
/// `rule`, or where and why it does not match, which the same function found
/// with `explain` tells.
pub fn parse(
    input: &str,
    rule: RuleFunction<false>,
    explain: RuleFunction<true>,
) -> Result<Tree, Error> {
    let mut parser = Parser::new(input);
    match rule(&mut parser, 0) {
        Some((end, node)) if end == input.len() => Ok(Tree {
            input: Arc::new(Input::new(input)),
            nodes: parser.nodes,
            children: parser.children,
            root: node.expect("a start rule is never hidden"),
        }),
        _ => {
            let mut parser = Parser::new(input);
            if let Some((end, _)) = explain(&mut parser, 0) {
                debug_assert!(end < input.len(), "both parses match alike");
                parser.note(end, Expected::End);
            }
            Err(parser.error())
        }
    }
}

/// The rules an input matched, each with the rules it matched inside it.
#[derive(Debug)]
pub struct Tree {
    input: Arc<Input>,
    nodes: Vec<NodeData>,
    /// The children of every node, those of each node next to each other.
    children: Vec<usize>,
    root: usize,
}

impl Tree {
    pub fn root(&self) -> Node<'_> {
        Node {
            tree: self,
            index: self.root,
        }
    }
}

#[derive(Debug)]
struct NodeData {
    rule: usize,
    start: usize,
    end: usize,
    /// Where the node's children are in the children of its tree.
    children: Range<usize>,
}

/// A rule an input matched, in its [`Tree`].
#[derive(Debug, Clone, Copy)]
pub struct Node<'t> {
    tree: &'t Tree,
    index: usize,
}

impl<'t> Node<'t> {
    fn data(&self) -> &'t NodeData {
        &self.tree.nodes[self.index]
    }

    /// Returns the number of the rule this node matched.
    pub fn rule(&self) -> usize {
        self.data().rule
    }

    pub fn span(&self) -> Span {
        let data = self.data();
        Span::new(Arc::clone(&self.tree.input), data.start..data.end)
    }

    pub fn text(&self) -> &'t str {
        let data = self.data();
        &self.tree.input.as_str()[data.start..data.end]
    }

    /// Returns the nodes of the rules matched inside this one, in order.
    pub fn children(&self) -> impl Iterator<Item = Node<'t>> + use<'t> {
        let tree = self.tree;
        tree.children[self.data().children.clone()]
            .iter()
            .map(move |&index| Node { tree, index })
    }

    /// Returns the only rule matched inside this one, if any.
    pub fn child(&self) -> Option<Node<'t>> {
        self.children().next()
    }

    /// Returns the rule numbered `rule` matched inside this one. The grammar
    /// makes sure it is there.
    pub fn one<T: FromNode>(&self, rule: usize) -> T {
        self.optional(rule)
            .expect("the grammar always matches this rule here")
    }

    /// Returns the rule numbered `rule` matched inside this one, if any.
    pub fn optional<T: FromNode>(&self, rule: usize) -> Option<T> {
        self.children()
            .find(|child| child.rule() == rule)
            .map(T::from_node)
    }

    /// Returns every rule numbered `rule` matched inside this one, in order.
    pub fn many<T: FromNode>(&self, rule: usize) -> Vec<T> {
        self.children()
            .filter(|child| child.rule() == rule)
            .map(T::from_node)
            .collect()
    }
}

/// Builds the type of a rule from the node of its match.
pub trait FromNode {
    fn from_node(node: Node<'_>) -> Self;
}

impl<T: FromNode> FromNode for Box<T> {
    fn from_node(node: Node<'_>) -> Self {
        Box::new(T::from_node(node))
    }
}

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
    fn new(input: &'i str) -> Self {
        Self {
            input,
            nodes: Vec::new(),
            children: Vec::new(),
            matched: Vec::new(),
            remembered: FxHashMap::default(),
            furthest: 0,
            expected: Vec::new(),
        }
    }

    fn error(self) -> Error {
        Error::new(self.input, self.furthest, self.expected)
    }

    /// Notes that `expected` could have been read at `position`.
    fn note(&mut self, position: usize, expected: Expected) {
        self.fail(position, &[expected]);
    }

    /// Returns the character at `position`, if the input goes on there.
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
        self.matched.extend(node);
        Some(end)
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
        // What the rule could not read where it starts is told as the rule
        // itself, unless it is hidden.
        let mut noted = false;
        if REPORTS
            && self.furthest == start.position
            && self.expected.len() > start.noted_before
        {
            self.expected.truncate(start.noted_before);
            if !rule.hidden {
                self.expected.push(Expected::Rule(rule.name));
                noted = true;
            }
        }
        if rule.remembers {
            let remembered = Remembered { found, noted };
            self.remembered
                .insert((rule.number, start.position), remembered);
        }
        found
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
        let first = self.children.len();
        self.children.extend(self.matched.drain(mark..));
        self.nodes.push(NodeData {
            rule: rule.number,
            start: range.start,
            end: range.end,
            children: first..self.children.len(),
        });
        Some(self.nodes.len() - 1)
    }

    /// Returns a parser of the input up to `end`, for an exception. An
    /// exception only reads what its factor matched, and its failures tell
    /// nothing about why the input does not match.
    pub fn within(&self, end: usize) -> Parser<'i, false> {
        Parser::new(&self.input[..end])
    }
}
