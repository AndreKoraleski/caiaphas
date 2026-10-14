//! Syntaxes of ISO/IEC 14977 EBNF.

use crate::parser::syntax_rule::SyntaxRule;

/// A syntax is every syntax rule of a grammar, in the order they are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Syntax {
    pub(crate) rules: Vec<SyntaxRule>,
}
