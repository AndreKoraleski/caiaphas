//! Rules of the intermediate representation.

use crate::{ir::Expression, lexer::MetaIdentifier, span::Span};

/// Identifies a rule by its index in the rules of its grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RuleId(pub(crate) usize);

/// A rule gives a meta identifier the expression it matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rule {
    pub(crate) meta_identifier: MetaIdentifier,
    pub(crate) definitions_list: Expression,
    /// The syntax rule it comes from.
    pub(crate) span: Span,
}
