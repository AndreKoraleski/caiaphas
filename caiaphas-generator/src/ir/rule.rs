//! Rules of the intermediate representation.

use crate::{
    annotation::{Expectation, Misspelling},
    ir::Expression,
    lexer::MetaIdentifier,
    span::Span,
};

/// Identifies a rule by its index in the rules of its grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuleId(pub(crate) usize);

/// Returns which of `count` rules can be reached from `starts`, where `next`
/// adds the rules a rule leads to onto the list of those still to visit.
pub(crate) fn reachable(
    count: usize,
    starts: impl IntoIterator<Item = RuleId>,
    mut next: impl FnMut(RuleId, &mut Vec<RuleId>),
) -> Vec<bool> {
    let mut reached = vec![false; count];
    let mut pending: Vec<RuleId> = starts.into_iter().collect();
    while let Some(rule) = pending.pop() {
        let RuleId(position) = rule;
        if !std::mem::replace(&mut reached[position], true) {
            next(rule, &mut pending);
        }
    }
    reached
}

/// A rule gives a meta identifier the expression it matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rule {
    pub(crate) meta_identifier: MetaIdentifier,
    /// What the rule matches, lowered from the definitions list of its syntax
    /// rule.
    pub(crate) expression: Expression,
    /// The findings the rule marks as intended.
    pub(crate) expectations: Vec<Expectation>,
    /// Whether a `(* hide *)` leaves the rule out of the generated types.
    pub(crate) hidden: bool,
    /// Whether a `(* greedy *)` right before the rule makes it take each way
    /// through it as it comes to it.
    pub(crate) greedy: bool,
    /// Whether a `(* recover *)` right before the rule lets a parse that
    /// recovers skip it ahead to its next match where it fails partway.
    pub(crate) recovers: bool,
    /// Whether an `(* inline *)` right before the rule gives the rules that
    /// hold it what it holds, instead of its type.
    pub(crate) inline: bool,
    /// The Rust type an `(* as T *)` right before the rule makes its type,
    /// read from the text it matched.
    pub(crate) converts_to: Option<String>,
    /// Each comment right before the rule of one word written almost as an
    /// annotation.
    pub(crate) misspellings: Vec<Misspelling>,
    /// The text of each comment right before the rule that is no annotation.
    /// The docs of the rule's type start with them.
    pub(crate) comments: Vec<String>,
    /// The syntax rule it comes from.
    pub(crate) span: Span,
}
