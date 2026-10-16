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

mod parser;
mod tree;

pub use parser::{Parser, RuleStart};
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
