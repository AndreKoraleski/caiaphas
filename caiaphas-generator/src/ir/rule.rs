//! Rules of the intermediate representation.

use crate::{
    annotation::Expectation, ir::Expression, lexer::MetaIdentifier, span::Span,
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
    /// Whether a hide leaves the rule out of the generated types.
    pub(crate) hidden: bool,
    /// Whether the choices written in the rule take their first alternative
    /// that matches, instead of the one that matches the most.
    pub(crate) first_match: bool,
    /// The syntax rule it comes from.
    pub(crate) span: Span,
}
