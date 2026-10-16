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

use crate::error::Error;
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
    /// Whether what the rule could not read goes unsaid, as for whitespace
    /// that can match nothing.
    pub quiet: bool,
    /// Whether the rule's node stands for its children in the tree, so the
    /// rule around it holds them instead.
    pub inline: bool,
    /// Whether the rule remembers its match at each position.
    pub remembers: bool,
    /// Whether no other rule uses the rule, so errors never call a whole input
    /// by its name.
    pub start: bool,
}

/// The function that matches a rule at a position.
pub type RuleFunction<const REPORTS: bool> =
    fn(&mut Parser<'_, REPORTS>, usize) -> Match;

/// Returns the tree of `input` matched in full by the rule whose function is
/// `rule`. When `input` does not match, `explain` matches it again to say
/// where and why.
pub fn parse(
    input: &str,
    rule: RuleFunction<false>,
    explain: RuleFunction<true>,
) -> Result<Tree, Error> {
    let mut parser = Parser::new(input);
    match rule(&mut parser, 0) {
        Some((end, node)) if end == input.len() => {
            Ok(parser.tree(node.expect("a start rule is never hidden")))
        }
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
    ends: EndsFunction<false>,
    build: BuildFunction,
    explain: EndsFunction<true>,
) -> Result<Tree, Error> {
    let mut parser = Parser::new(input);
    if ends(&mut parser, 0).contains(input.len()) {
        let root = build(&mut parser, 0, input.len());
        return Ok(parser.tree(root.expect("a start rule is never hidden")));
    }
    let mut parser = Parser::new(input);
    for end in explain(&mut parser, 0).iter() {
        debug_assert!(end < input.len(), "both parses match alike");
        parser.note(end, Expected::End);
    }
    Err(parser.error())
}
