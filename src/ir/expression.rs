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
    /// Matches `syntactic_primary` exactly `integer` times. Lowering only
    /// makes one when `integer` is two or more.
    SyntacticFactor {
        integer: u32,
        syntactic_primary: Box<Expression>,
    },
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

impl Expression {
    /// Returns the expressions directly inside this one.
    pub(crate) fn children(&self) -> impl Iterator<Item = &Self> {
        let (list, first, second): (&[Self], Option<&Self>, Option<&Self>) =
            match &self.kind {
                ExpressionKind::SingleDefinition(expressions)
                | ExpressionKind::DefinitionsList(expressions) => {
                    (expressions, None, None)
                }
                ExpressionKind::OptionalSequence(expression)
                | ExpressionKind::RepeatedSequence(expression)
                | ExpressionKind::SyntacticFactor {
                    syntactic_primary: expression,
                    ..
                } => (&[], Some(expression), None),
                ExpressionKind::SyntacticTerm {
                    syntactic_factor,
                    syntactic_exception,
                } => (&[], Some(syntactic_factor), Some(syntactic_exception)),
                ExpressionKind::MetaIdentifier(_)
                | ExpressionKind::TerminalString(_)
                | ExpressionKind::SpecialSequence(_)
                | ExpressionKind::EmptySequence => (&[], None, None),
            };
        list.iter().chain(first).chain(second)
    }

    /// Calls `visit` with this expression and every expression inside it, each
    /// before the ones inside it.
    pub(crate) fn visit(&self, visit: &mut impl FnMut(&Self)) {
        visit(self);
        for child in self.children() {
            child.visit(visit);
        }
    }

    /// Calls `visit` with the rule and span of every meta identifier in this
    /// expression.
    pub(crate) fn meta_identifiers(
        &self,
        visit: &mut impl FnMut(RuleId, Span),
    ) {
        self.visit(&mut |expression| {
            if let ExpressionKind::MetaIdentifier(rule_id) = expression.kind {
                visit(rule_id, expression.span);
            }
        });
    }
}
