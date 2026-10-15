//! Expressions of the intermediate representation.

use std::ops::RangeInclusive;

use crate::{ir::RuleId, span::Span};

/// An expression is what a rule, or part of one, matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expression {
    pub(crate) kind: ExpressionKind,
    /// The part of the syntax it comes from.
    pub(crate) span: Span,
}

/// What an [`Expression`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExpressionKind {
    /// Matches each expression one after another.
    SingleDefinition(Vec<Expression>),
    /// Matches any one of the expressions.
    DefinitionsList(Vec<Expression>),
    /// Matches the expression once or not at all.
    OptionalSequence(Box<Expression>),
    /// Matches the expression any number of times.
    RepeatedSequence(Box<Expression>),
    /// Matches what `syntactic_factor` matches, unless `syntactic_exception`
    /// matches it too.
    SyntacticTerm {
        syntactic_factor: Box<Expression>,
        syntactic_exception: Box<Expression>,
    },
    /// Matches what the rule it names matches.
    MetaIdentifier(RuleId),
    /// Matches these characters exactly.
    TerminalString(String),
    /// Matches any one character in the range.
    SpecialSequence(RangeInclusive<char>),
    /// Matches without reading anything.
    EmptySequence,
}
