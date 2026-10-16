//! Evaluates arithmetic with the types generated from its grammar.

caiaphas::grammar!(mod arithmetic = "tests/grammars/arithmetic.ebnf");

use arithmetic::{Arithmetic, Expression, Factor, Term};

fn evaluate(expression: &Expression) -> i64 {
    let mut terms = expression.term.iter().map(term);
    let first = terms.next().unwrap_or_default();
    expression.additive_operator.iter().zip(terms).fold(
        first,
        |total, (operator, value)| match operator.text() {
            "+" => total + value,
            _ => total - value,
        },
    )
}

fn term(term: &Term) -> i64 {
    let mut factors = term.factor.iter().map(factor);
    let first = factors.next().unwrap_or_default();
    term.multiplicative_operator.iter().zip(factors).fold(
        first,
        |total, (operator, value)| match operator.text() {
            "*" => total * value,
            _ => total / value,
        },
    )
}

fn factor(factor: &Factor) -> i64 {
    match (&factor.number, &factor.expression) {
        (Some(number), _) => number.text().parse().unwrap(),
        (None, Some(expression)) => evaluate(expression),
        (None, None) => unreachable!("a factor is a number or a group"),
    }
}

#[test]
fn evaluates_with_the_types_of_the_rules() {
    let arithmetic = Arithmetic::parse("2*(3+4)-10/5").unwrap();
    assert_eq!(evaluate(&arithmetic.expression), 12);
}

#[test]
fn rejects_an_unclosed_group() {
    let error = Arithmetic::parse("(1+2").unwrap_err();
    assert_eq!(error.line(), 1);
    assert_eq!(error.column(), 5);
}
