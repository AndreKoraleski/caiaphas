//! The intermediate representation of a grammar, as analysis and code
//! generation read it.

mod expression;
mod rule;

pub(crate) use expression::{Expression, ExpressionKind};
pub(crate) use rule::{Rule, RuleId};

/// A grammar is its rules, with every reference between them resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Grammar {
    /// The rules in the order they are written, so a [`RuleId`] is an index.
    pub(crate) syntax_rules: Vec<Rule>,
    /// The rules no other rule references. Each gets its own parser.
    pub(crate) start_syntax_rules: Vec<RuleId>,
}
