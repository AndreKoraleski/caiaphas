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
//! A start that can end at more than one place stays where it is, since
//! written once it would take the most text before the choice after it.

use crate::{
    analysis::Facts,
    ir::{Expression, ExpressionKind, Grammar},
    span::Span,
};

/// Returns `grammar` with its shared starts written once.
pub(super) fn rearranged(grammar: &Grammar) -> Grammar {
    let facts = Facts::new(grammar);
    let mut rearranged = grammar.clone();
    for (rule, written) in rearranged
        .syntax_rules
        .iter_mut()
        .zip(&grammar.syntax_rules)
    {
        rule.expression = expression(written.expression.clone(), &facts);
    }
    rearranged
}

/// Returns `expression` with its shared starts written once.
fn expression(expression: Expression, facts: &Facts<'_>) -> Expression {
    let span = expression.span;
    let boxed = |expression: Box<Expression>| {
        Box::new(self::expression(*expression, facts))
    };
    let kind = match expression.kind {
        ExpressionKind::DefinitionsList(alternatives) => {
            let alternatives = alternatives
                .into_iter()
                .map(|alternative| self::expression(alternative, facts))
                .collect();
            return choice(alternatives, span, facts);
        }
        ExpressionKind::SingleDefinition(steps) => {
            ExpressionKind::SingleDefinition(
                steps
                    .into_iter()
                    .map(|step| self::expression(step, facts))
                    .collect(),
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
fn choice(
    alternatives: Vec<Expression>,
    span: Span,
    facts: &Facts<'_>,
) -> Expression {
    let mut joined = Vec::new();
    let mut alternatives = alternatives.into_iter().peekable();
    while let Some(first) = alternatives.next() {
        let start = &first.steps()[0];
        // An empty start would leave empty rests to join again forever.
        let joins = facts.ends_once(start)
            && !matches!(start.kind, ExpressionKind::EmptySequence);
        let mut run = vec![first];
        while let Some(next) = alternatives.next_if(|next| {
            joins && next.steps()[0].is_written_like(&run[0].steps()[0])
        }) {
            run.push(next);
        }
        joined.push(if run.len() == 1 {
            run.remove(0)
        } else {
            factored(run, facts)
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

/// Returns the alternatives of `run` as the expression they all start with,
/// followed by a choice of what each has after it.
fn factored(run: Vec<Expression>, facts: &Facts<'_>) -> Expression {
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
    let rest = choice(rests, span, facts);
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

/// Returns `steps` as one expression. With no steps, it matches nothing at
/// `end`.
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
        render(&rearranged(&build_valid(source)), name)
    }

    #[test]
    fn leaves_a_start_that_can_end_at_more_than_one_place() {
        assert_eq!(
            rearranged_rule("s = a, 'ab' | a, 'b'; a = {'a'};", "s"),
            "alt(seq(a, 'ab'), seq(a, 'b'))"
        );
    }

    #[test]
    fn joins_duplicate_alternatives_once() {
        assert_eq!(
            rearranged_rule("s = 'x' | 'x';", "s"),
            "seq('x', alt(empty, empty))"
        );
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
