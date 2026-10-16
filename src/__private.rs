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
//! Taking one way through each choice as it comes can leave out the way the
//! rest of the input needs. A rule where it could gets two functions that try
//! every way instead. One tells every place where the rule's matches at a
//! position can end, in the order the rule prefers them, and remembers them
//! there. The other builds the node of the match that ends where the rule
//! around it picks. Each part of a match takes the first end it prefers that
//! the rest of the match can go on from, and the start rule takes the end of
//! the input. A rule prefers the end of the most text, unless it takes the
//! first match, when it prefers each way by the first branch it takes, then by
//! what comes after. The analysis tells which rules need this. Every other
//! rule has at most one end the rest of an input can go on from, and its
//! function finds it.
//!
//! A parse notes nothing about why an input might not match, and skips the
//! branches that cannot start with the next character. Only when the input
//! does not match is it matched again, trying every branch and noting at each
//! failure what could have been read, to say why. The function of each rule is
//! generic over which of the two it does, so the first leaves out the work of
//! the second entirely.

mod parser;
mod tree;

pub use parser::{Ends, Parser, RuleStart};
pub use tree::{FromNode, Node, Tree};

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
/// `ends` and `build`, or where and why it does not match. `explain` tells
/// that as `ends` does.
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
