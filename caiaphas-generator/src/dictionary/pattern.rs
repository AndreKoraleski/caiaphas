//! A special sequence's pattern becomes the expressions of the grammar it
//! stands for, so the analysis and the parser read it as they read any other.

use crate::{
    ir::{Expression, ExpressionKind},
    lexer::Pattern,
    span::Span,
};

/// Returns the expression `pattern` stands for, written at `span`.
pub(super) fn expression(pattern: &Pattern, span: Span) -> Expression {
    let at = |kind| Expression { kind, span };
    match pattern {
        Pattern::Characters(ranges) => {
            let mut sets: Vec<Expression> = ranges
                .iter()
                .map(|range| {
                    at(ExpressionKind::SpecialSequence(range.clone()))
                })
                .collect();
            if sets.len() == 1 {
                sets.remove(0)
            } else {
                at(ExpressionKind::DefinitionsList(sets))
            }
        }
        Pattern::Sequence(steps) => sequence(
            steps.iter().map(|step| expression(step, span)).collect(),
            span,
        ),
        Pattern::Choice(alternatives) => at(ExpressionKind::DefinitionsList(
            alternatives
                .iter()
                .map(|alternative| expression(alternative, span))
                .collect(),
        )),
        Pattern::Repeat {
            pattern,
            least,
            most,
        } => repeat(&expression(pattern, span), *least, *most, span),
    }
}

/// Returns `once` at least `least` times, and at most `most` when it is set.
/// Each optional time holds the next, so `x{0,2}` is `[x, [x]]`, which reads
/// one way only.
fn repeat(
    once: &Expression,
    least: u32,
    most: Option<u32>,
    span: Span,
) -> Expression {
    let at = |kind| Expression { kind, span };
    let mut steps = Vec::new();
    match least {
        0 => {}
        1 => steps.push(once.clone()),
        integer => steps.push(at(ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary: Box::new(once.clone()),
        })),
    }
    match most {
        None => steps.push(at(ExpressionKind::RepeatedSequence(Box::new(
            once.clone(),
        )))),
        Some(most) => {
            let mut optional: Option<Expression> = None;
            for _ in least..most {
                let inner = match optional.take() {
                    Some(next) => sequence(vec![once.clone(), next], span),
                    None => once.clone(),
                };
                optional = Some(at(ExpressionKind::OptionalSequence(
                    Box::new(inner),
                )));
            }
            steps.extend(optional);
        }
    }
    sequence(steps, span)
}

/// Returns `steps` one after another, with the steps of a sequence among them
/// taken in as steps of this one.
fn sequence(steps: Vec<Expression>, span: Span) -> Expression {
    let mut flat = Vec::new();
    for step in steps {
        match step.kind {
            ExpressionKind::SingleDefinition(inner) => flat.extend(inner),
            _ => flat.push(step),
        }
    }
    match flat.len() {
        0 => Expression {
            kind: ExpressionKind::EmptySequence,
            span,
        },
        1 => flat.remove(0),
        _ => Expression {
            kind: ExpressionKind::SingleDefinition(flat),
            span,
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::dictionary::tests::{build_valid, render};

    #[test]
    fn writes_a_pattern_as_expressions() {
        let grammar = build_valid("s = ? /[a-c]x{2,3}(y|z)*/ ?;");
        assert_eq!(
            render(&grammar, "s"),
            "seq(U+0061..U+0063, 2*U+0078..U+0078, opt(U+0078..U+0078), \
             rep(alt(U+0079..U+0079, U+007A..U+007A)))"
        );
    }
}
