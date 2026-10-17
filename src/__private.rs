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

mod nodes;
mod parser;
mod tree;

pub use nodes::{Nodes, Store, Width};
use parser::Recoveries;
pub use parser::{Ends, Parser, RuleStart};
pub use tree::{Converted, FromNode, Node, Tree};

pub use crate::error::Expected;
use crate::{Recovered, error::Error};

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

/// The start rule a parse matches the whole input with. Each grammar compiles
/// it for each width of node a parser can have.
pub trait Start {
    /// Returns the root node of the input of `parser`, when the rule matches
    /// all of it.
    fn root<W: Width>(parser: &mut Parser<'_, false, W>) -> Option<usize>;

    /// Matches the input of `parser` again, noting why the rule does not match
    /// all of it, and returns where its matches end.
    fn explain(parser: &mut Parser<'_, true, u64>) -> Vec<usize>;
}

/// Returns the root node of an input of `length` bytes, when `found` is a
/// match of all of it.
#[must_use]
pub fn whole(found: Match, length: usize) -> Option<usize> {
    match found {
        Some((end, node)) if end == length => node,
        _ => None,
    }
}

/// Returns the tree of `input` matched in full by the start rule `S`. When
/// `input` does not match, the rule matches it again to say where and why. The
/// parser follows `depth` matches of recursive rules inside one another.
pub fn parse<S: Start>(input: &str, depth: usize) -> Result<Tree, Error> {
    if let Some(tree) = matched::<S>(input, depth, &Recoveries::default()) {
        return Ok(tree);
    }
    let mut parser = Parser::new(input, depth);
    for end in S::explain(&mut parser) {
        debug_assert!(end < input.len(), "both parses match alike");
        parser.note(end, Expected::End);
    }
    Err(parser.error())
}

/// Returns what [`parse`] does, but goes on past each error that a rule that
/// recovers lets the parse skip, and gives every error found. The tree comes
/// when the input matches once those errors are skipped.
#[must_use]
pub fn parse_recovering<S: Start>(
    input: &str,
    depth: usize,
) -> (Option<Tree>, Vec<Error>) {
    parser::recovering::<S>(input, depth)
}

/// Returns the tree of `input` when the start rule `S` matches all of it,
/// built by the parser of the narrowest width that holds its nodes. Rules that
/// recover go on as `recoveries` tells.
pub(crate) fn matched<S: Start>(
    input: &str,
    depth: usize,
    recoveries: &Recoveries,
) -> Option<Tree> {
    let length = input.len();
    if nodes::fits::<u8>(length) {
        if let Ok(tree) = matched_in::<S, u8>(input, depth, recoveries) {
            return tree;
        }
    }
    if nodes::fits::<u16>(length) {
        if let Ok(tree) = matched_in::<S, u16>(input, depth, recoveries) {
            return tree;
        }
    }
    if nodes::fits::<u32>(length) {
        if let Ok(tree) = matched_in::<S, u32>(input, depth, recoveries) {
            return tree;
        }
    }
    matched_in::<S, u64>(input, depth, recoveries)
        .ok()
        .flatten()
}

/// Returns what [`matched`] does with a parser of width `W`, or `Err` when its
/// nodes outgrow the width.
fn matched_in<S: Start, W: Width>(
    input: &str,
    depth: usize,
    recoveries: &Recoveries,
) -> Result<Option<Tree>, ()> {
    let mut parser = Parser::<false, W>::new(input, depth);
    parser.recover_as(recoveries.clone());
    let root = S::root(&mut parser);
    if parser.overflowed() {
        return Err(());
    }
    if parser.went_too_deep() {
        return Ok(None);
    }
    Ok(root.map(|root| parser.tree(root)))
}

/// Returns the value and errors of a parse that recovers.
pub fn recovered<T>(value: Option<T>, errors: Vec<Error>) -> Recovered<T> {
    Recovered::new(value, errors)
}
