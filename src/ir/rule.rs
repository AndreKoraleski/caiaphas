//! Rules of the intermediate representation.

use crate::{
    annotation::Expectation, ir::Expression, lexer::MetaIdentifier, span::Span,
};

/// Identifies a rule by its index in the rules of its grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RuleId(pub(crate) usize);

/// A rule gives a meta identifier the expression it matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rule {
    pub(crate) meta_identifier: MetaIdentifier,
    /// What the definitions list of the syntax rule lowers to, which is only a
    /// definitions list when it has more than one single definition.
    pub(crate) definitions_list: Expression,
    /// The findings the rule marks as intended.
    pub(crate) expectations: Vec<Expectation>,
    /// Whether a hide leaves the rule out of the generated types.
    pub(crate) hidden: bool,
    /// The syntax rule it comes from.
    pub(crate) span: Span,
}
