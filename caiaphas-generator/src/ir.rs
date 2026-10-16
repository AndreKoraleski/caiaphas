//! The intermediate representation of a grammar, as analysis and code
//! generation read it.

mod expression;
mod rule;

pub(crate) use expression::{Expression, ExpressionKind};
pub(crate) use rule::{Rule, RuleId};

/// A grammar is its rules, with every reference between them resolved.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Grammar {
    /// The rules in the order they are written, without the later
    /// definitions of a meta identifier.
    pub(crate) syntax_rules: Vec<Rule>,
    /// The rules no other rule references. Each gets its own parser.
    pub(crate) start_syntax_rules: Vec<RuleId>,
}

impl Grammar {
    /// Calls `visit` with every expression of every rule, each before the ones
    /// inside it.
    pub(crate) fn visit(&self, visit: &mut impl FnMut(&Expression)) {
        for rule in &self.syntax_rules {
            rule.expression.visit(visit);
        }
    }
}
