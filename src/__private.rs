//! What generated parsers are made of. Nothing here is meant to be used
//! directly, so it can change in any release.
//!
//! Generated code matches each rule with a [`Parser`] and turns the tree it
//! builds into the grammar's types. Most rules take each way as it comes. A
//! rule where that could miss a match gets two functions instead: one lists
//! where its matches can end, and the other builds the match that the rest of
//! the input goes on from.
//!
//! A first parse notes nothing about failures. Only an input that does not
//! match is parsed again, noting what could have come at each failure, to say
//! why. The function of each rule is generic over which of the two it does.

mod parser;
mod tree;

pub use parser::{Ends, Parser, RuleStart};
pub use tree::{Converted, FromNode, Node, Tree};

use std::{
    fmt,
    hash::{Hash, Hasher},
};

pub use crate::error::Expected;
use crate::{Recovered, Span, error::Error};

/// Where a rule's match ends, with its node unless the rule is hidden, or
/// `None` when it does not match.
pub type Match = Option<(usize, Option<usize>)>;

/// What a parser needs to know about a rule besides how to match it.
#[derive(Debug)]
pub struct Rule {
    pub number: usize,
    /// The name of the rule as written. Errors call the rule by it.
    pub name: &'static str,
    /// Whether the rule is left out of the tree, along with what it matches,
    /// and mostly out of errors.
    pub hidden: bool,
    /// Whether what the rule could not read goes unsaid, as for whitespace
    /// that can match nothing.
    pub quiet: bool,
    /// Whether the rule builds a node, since a type reads it. A hidden rule,
    /// and one that only a converted rule or no type holds, builds none.
    pub builds: bool,
    /// Whether the rule's node stands for its children in the tree, so the
    /// rule around it holds them instead.
    pub inline: bool,
    /// Whether the rule remembers its match at each position.
    pub remembers: bool,
    /// Whether no other rule uses the rule, so errors never call a whole input
    /// by its name.
    pub start: bool,
}

/// How many matches of recursive rules a parser follows inside one another,
/// unless its grammar's `depth` says otherwise. Each takes some stack, and
/// this many fit in the smallest stack a program usually has, even in a build
/// that is not optimized.
pub const DEPTH: usize = 512;

/// The function that matches a rule at a position.
pub type RuleFunction<const REPORTS: bool> =
    fn(&mut Parser<'_, REPORTS>, usize) -> Match;

/// Returns the tree of `input` matched in full by the rule whose function is
/// `rule`. When `input` does not match, `explain` matches it again to say
/// where and why.
pub fn parse(
    input: &str,
    depth: usize,
    rule: RuleFunction<false>,
    explain: RuleFunction<true>,
) -> Result<Tree, Error> {
    let mut parser = Parser::new(input, depth);
    match rule(&mut parser, 0) {
        Some((end, node)) if end == input.len() && !parser.went_too_deep() => {
            Ok(parser.tree(node.expect("a start rule is never hidden")))
        }
        _ => {
            let mut parser = Parser::new(input, depth);
            if let Some((end, _)) = explain(&mut parser, 0) {
                debug_assert!(end < input.len(), "both parses match alike");
                parser.note(end, Expected::End);
            }
            Err(parser.error())
        }
    }
}

/// The function of a rule that tells where its matches at a position can end.
pub type EndsFunction<const REPORTS: bool> =
    fn(&mut Parser<'_, REPORTS>, usize) -> Ends;

/// The function of a rule that builds the node of its match from a position to
/// an end it can have there. A hidden rule has no node.
pub type BuildFunction =
    fn(&mut Parser<'_, false>, usize, usize) -> Option<usize>;

/// Returns the tree of `input` matched in full by the rule whose functions are
/// `ends` and `build`. When `input` does not match, `explain` matches it again
/// as `ends` does to say where and why.
pub fn parse_by_ends(
    input: &str,
    depth: usize,
    ends: EndsFunction<false>,
    build: BuildFunction,
    explain: EndsFunction<true>,
) -> Result<Tree, Error> {
    let mut parser = Parser::new(input, depth);
    if ends(&mut parser, 0).contains(input.len()) && !parser.went_too_deep() {
        let root = build(&mut parser, 0, input.len());
        return Ok(parser.tree(root.expect("a start rule is never hidden")));
    }
    let mut parser = Parser::new(input, depth);
    for end in explain(&mut parser, 0).iter() {
        debug_assert!(end < input.len(), "both parses match alike");
        parser.note(end, Expected::End);
    }
    Err(parser.error())
}

/// Returns what [`parse`] does, but goes on past each error that a rule that
/// recovers lets the parse skip, and gives every error found. The tree comes
/// when the input matches once those errors are skipped.
pub fn parse_recovering(
    input: &str,
    depth: usize,
    rule: RuleFunction<false>,
    explain: RuleFunction<true>,
) -> (Option<Tree>, Vec<Error>) {
    parser::recovering(
        input,
        depth,
        |parser| match rule(parser, 0) {
            Some((end, node)) if end == input.len() => node,
            _ => None,
        },
        |parser| explain(parser, 0).map(|(end, _)| end).into_iter().collect(),
    )
}

/// Returns what [`parse_by_ends`] does, going on past errors as
/// [`parse_recovering`] does.
pub fn parse_by_ends_recovering(
    input: &str,
    depth: usize,
    ends: EndsFunction<false>,
    build: BuildFunction,
    explain: EndsFunction<true>,
) -> (Option<Tree>, Vec<Error>) {
    parser::recovering(
        input,
        depth,
        |parser| {
            ends(parser, 0)
                .contains(input.len())
                .then(|| build(parser, 0, input.len()))
                .flatten()
        },
        |parser| explain(parser, 0).iter().collect(),
    )
}

/// Returns the value and errors of a parse that recovers.
pub fn recovered<T>(value: Option<T>, errors: Vec<Error>) -> Recovered<T> {
    Recovered::new(value, errors)
}

/// The span of a generated struct. It compares and hashes by its text alone,
/// so two matches of the same text are equal wherever they are.
#[derive(Clone)]
pub struct TextSpan(pub Span);

impl PartialEq for TextSpan {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for TextSpan {}

impl Hash for TextSpan {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl fmt::Debug for TextSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
