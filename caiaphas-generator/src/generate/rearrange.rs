//! Rearranging rewrites a grammar into one that matches the same input and
//! builds the same nodes in the same order, but that its parser reads faster.
//! A grammar is only rearranged when its `grammar!` asks for it, since each
//! rearrangement could be written in the grammar, as a suggestion tells.
//!
//! Neighbouring alternatives that start with the same expression become that
//! start followed by a choice of what each has after it, so `a, x | a, y`
//! becomes `a, (x | y)`. The start then matches once, and the rules in it no
//! longer have to remember their matches. Only neighbours are joined, since
//! moving an alternative past another could change which of two that tie wins.

use crate::{
    ir::{Expression, ExpressionKind, Grammar, Rule},
    span::Span,
};

/// Returns `grammar` with its shared starts written once.
pub(super) fn rearranged(grammar: Grammar) -> Grammar {
    let syntax_rules = grammar
        .syntax_rules
        .into_iter()
        .map(|rule| Rule {
            expression: expression(rule.expression),
            ..rule
        })
        .collect();
    Grammar {
        syntax_rules,
        ..grammar
    }
}

/// Returns `expression` with its shared starts written once.
fn expression(expression: Expression) -> Expression {
    let span = expression.span;
    let boxed =
        |expression: Box<Expression>| Box::new(self::expression(*expression));
    let kind = match expression.kind {
        ExpressionKind::DefinitionsList(alternatives) => {
            let alternatives =
                alternatives.into_iter().map(self::expression).collect();
            return choice(alternatives, span);
        }
        ExpressionKind::SingleDefinition(steps) => {
            ExpressionKind::SingleDefinition(
                steps.into_iter().map(self::expression).collect(),
            )
        }
        ExpressionKind::OptionalSequence(body) => {
            ExpressionKind::OptionalSequence(boxed(body))
        }
        ExpressionKind::RepeatedSequence(body) => {
            ExpressionKind::RepeatedSequence(boxed(body))
        }
        ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary,
        } => ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary: boxed(syntactic_primary),
        },
        ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } => ExpressionKind::SyntacticTerm {
            syntactic_factor: boxed(syntactic_factor),
            syntactic_exception: boxed(syntactic_exception),
        },
        kind => kind,
    };
    Expression { kind, span }
}

/// Returns the choice of `alternatives`, written at `span`, with each run of
/// neighbours that start with the same expression joined.
fn choice(alternatives: Vec<Expression>, span: Span) -> Expression {
    let mut joined = Vec::new();
    let mut alternatives = alternatives.into_iter().peekable();
    while let Some(first) = alternatives.next() {
        let mut run = vec![first];
        while let Some(next) = alternatives.next_if(|next| {
            next.steps()[0].is_written_like(&run[0].steps()[0])
        }) {
            run.push(next);
        }
        joined.push(if run.len() == 1 {
            run.remove(0)
        } else {
            factored(run)
        });
    }
    if joined.len() == 1 {
        return joined.remove(0);
    }
    Expression {
        kind: ExpressionKind::DefinitionsList(joined),
        span,
    }
}

/// Returns the alternatives of `run`, which all start with the same
/// expression, as that start followed by a choice of what each has after it.
fn factored(run: Vec<Expression>) -> Expression {
    let span = Span {
        start: run[0].span.start,
        end: run[run.len() - 1].span.end,
    };
    let mut start = None;
    let rests = run
        .into_iter()
        .map(|alternative| {
            let end = alternative.span.end;
            let mut steps = into_steps(alternative);
            start.get_or_insert(steps.remove(0));
            sequence(steps, end)
        })
        .collect();
    let mut steps = vec![start.expect("a run has alternatives")];
    let rest = choice(rests, span);
    match rest.kind {
        ExpressionKind::SingleDefinition(more) => steps.extend(more),
        _ => steps.push(rest),
    }
    Expression {
        kind: ExpressionKind::SingleDefinition(steps),
        span,
    }
}

/// Returns the expressions `expression` matches one after another.
fn into_steps(expression: Expression) -> Vec<Expression> {
    match expression.kind {
        ExpressionKind::SingleDefinition(steps) => steps,
        _ => vec![expression],
    }
}

/// Returns `steps` as one expression, which matches nothing at `end` when
/// there are none.
fn sequence(mut steps: Vec<Expression>, end: usize) -> Expression {
    match steps.len() {
        0 => Expression {
            kind: ExpressionKind::EmptySequence,
            span: Span { start: end, end },
        },
        1 => steps.remove(0),
        _ => Expression {
            span: Span {
                start: steps[0].span.start,
                end,
            },
            kind: ExpressionKind::SingleDefinition(steps),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::{build_valid, render};

    fn rearranged_rule(source: &str, name: &str) -> String {
        render(&rearranged(build_valid(source)), name)
    }

    #[test]
    fn writes_a_shared_start_once() {
        assert_eq!(
            rearranged_rule("s = a, 'x' | a, 'y'; a = 'z';", "s"),
            "seq(a, alt('x', 'y'))"
        );
        assert_eq!(
            rearranged_rule("s = a | a, 'y'; a = 'z';", "s"),
            "seq(a, alt(empty, 'y'))"
        );
    }

    #[test]
    fn writes_a_longer_shared_start_once() {
        assert_eq!(
            rearranged_rule("s = a, a, 'x' | a, a, 'y'; a = 'z';", "s"),
            "seq(a, a, alt('x', 'y'))"
        );
    }

    #[test]
    fn joins_only_neighbours() {
        assert_eq!(
            rearranged_rule("s = a, 'x' | 'w' | a, 'y'; a = 'z';", "s"),
            "alt(seq(a, 'x'), 'w', seq(a, 'y'))"
        );
        assert_eq!(
            rearranged_rule("s = 'w' | a, 'x' | a, 'y'; a = 'z';", "s"),
            "alt('w', seq(a, alt('x', 'y')))"
        );
    }

    #[test]
    fn rearranges_inside_other_expressions() {
        assert_eq!(
            rearranged_rule("s = {a, 'x' | a, 'y'}; a = 'z';", "s"),
            "rep(seq(a, alt('x', 'y')))"
        );
    }
}
