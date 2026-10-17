//! Expressions of the intermediate representation.

use std::ops::RangeInclusive;

use crate::{
    ir::{Grammar, RuleId},
    span::Span,
};

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
    /// Matches nothing, where `expression` matches none of what comes next.
    Not(Box<Expression>),
    /// Matches without reading anything.
    EmptySequence,
}

impl Expression {
    /// Returns the expressions this one matches one after another: the steps
    /// of a single definition, or the expression alone.
    pub(crate) fn steps(&self) -> &[Self] {
        match &self.kind {
            ExpressionKind::SingleDefinition(steps) => steps,
            _ => std::slice::from_ref(self),
        }
    }

    /// Returns what this expression goes on with after `shorter`, when it
    /// starts with every expression of `shorter` and has more.
    pub(crate) fn rest_after(&self, shorter: &Self) -> Option<&[Self]> {
        let (shorter, longer) = (shorter.steps(), self.steps());
        let starts_alike = shorter
            .iter()
            .zip(longer)
            .all(|(shorter, longer)| shorter.is_written_like(longer));
        (starts_alike && longer.len() > shorter.len())
            .then(|| &longer[shorter.len()..])
    }

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
                | ExpressionKind::Not(expression)
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
    pub(crate) fn visit_meta_identifiers(
        &self,
        visit: &mut impl FnMut(RuleId, Span),
    ) {
        self.visit(&mut |expression| {
            if let ExpressionKind::MetaIdentifier(rule_id) = expression.kind {
                visit(rule_id, expression.span);
            }
        });
    }

    /// Returns the expression written in the syntax of the standard, with the
    /// names of the rules of `grammar`, unless it holds a special sequence,
    /// which lowering leaves unreadable.
    pub(crate) fn written(&self, grammar: &Grammar) -> Option<String> {
        self.written_at(grammar, Binding::Definitions)
    }

    /// Returns what [`written`](Self::written) does, in brackets when the
    /// expression binds more loosely than `binding` allows.
    fn written_at(
        &self,
        grammar: &Grammar,
        binding: Binding,
    ) -> Option<String> {
        let all = |expressions: &[Self], binding, apart| {
            let parts: Option<Vec<String>> = expressions
                .iter()
                .map(|expression| expression.written_at(grammar, binding))
                .collect();
            Some(parts?.join(apart))
        };
        let (text, binds) = match &self.kind {
            ExpressionKind::DefinitionsList(alternatives) => (
                all(alternatives, Binding::Definition, " | ")?,
                Binding::Definitions,
            ),
            ExpressionKind::SingleDefinition(steps) => {
                (all(steps, Binding::Term, ", ")?, Binding::Definition)
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => (
                format!(
                    "{} - {}",
                    syntactic_factor.written_at(grammar, Binding::Factor)?,
                    syntactic_exception
                        .written_at(grammar, Binding::Factor)?
                ),
                Binding::Term,
            ),
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => (
                format!(
                    "{integer} * {}",
                    syntactic_primary.written_at(grammar, Binding::Primary)?
                ),
                Binding::Factor,
            ),
            ExpressionKind::OptionalSequence(sequence) => (
                format!("[{}]", sequence.written(grammar)?),
                Binding::Primary,
            ),
            ExpressionKind::RepeatedSequence(sequence) => (
                format!("{{{}}}", sequence.written(grammar)?),
                Binding::Primary,
            ),
            ExpressionKind::MetaIdentifier(RuleId(position)) => (
                grammar.syntax_rules[*position].meta_identifier.spaced(),
                Binding::Primary,
            ),
            ExpressionKind::TerminalString(text) if text.contains('\'') => {
                (format!("\"{text}\""), Binding::Primary)
            }
            ExpressionKind::TerminalString(text) => {
                (format!("'{text}'"), Binding::Primary)
            }
            ExpressionKind::EmptySequence => (String::new(), Binding::Primary),
            ExpressionKind::SpecialSequence(_) | ExpressionKind::Not(_) => {
                return None;
            }
        };
        Some(if binds < binding {
            format!("({text})")
        } else {
            text
        })
    }

    /// Returns whether this expression is written the same way as `other`,
    /// wherever each is.
    pub(crate) fn is_written_like(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (
                ExpressionKind::SingleDefinition(left),
                ExpressionKind::SingleDefinition(right),
            )
            | (
                ExpressionKind::DefinitionsList(left),
                ExpressionKind::DefinitionsList(right),
            ) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| left.is_written_like(right))
            }
            (
                ExpressionKind::OptionalSequence(left),
                ExpressionKind::OptionalSequence(right),
            )
            | (
                ExpressionKind::RepeatedSequence(left),
                ExpressionKind::RepeatedSequence(right),
            )
            | (ExpressionKind::Not(left), ExpressionKind::Not(right)) => {
                left.is_written_like(right)
            }
            (
                ExpressionKind::SyntacticFactor {
                    integer: left_integer,
                    syntactic_primary: left,
                },
                ExpressionKind::SyntacticFactor {
                    integer: right_integer,
                    syntactic_primary: right,
                },
            ) => left_integer == right_integer && left.is_written_like(right),
            (
                ExpressionKind::SyntacticTerm {
                    syntactic_factor: left_factor,
                    syntactic_exception: left_exception,
                },
                ExpressionKind::SyntacticTerm {
                    syntactic_factor: right_factor,
                    syntactic_exception: right_exception,
                },
            ) => {
                left_factor.is_written_like(right_factor)
                    && left_exception.is_written_like(right_exception)
            }
            (
                ExpressionKind::MetaIdentifier(left),
                ExpressionKind::MetaIdentifier(right),
            ) => left == right,
            (
                ExpressionKind::TerminalString(left),
                ExpressionKind::TerminalString(right),
            ) => left == right,
            (
                ExpressionKind::SpecialSequence(left),
                ExpressionKind::SpecialSequence(right),
            ) => left == right,
            (ExpressionKind::EmptySequence, ExpressionKind::EmptySequence) => {
                true
            }
            _ => false,
        }
    }
}

/// How tightly the parts of an expression bind, from the loosest, so the
/// standard's syntax only needs brackets around a looser part in a tighter
/// place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Binding {
    Definitions,
    Definition,
    Term,
    Factor,
    Primary,
}
