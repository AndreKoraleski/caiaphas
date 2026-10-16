//! Works out arithmetic given on the command line:
//!
//! ```text
//! cargo run --example calculator -- "1.5 + 2 * (3 - 4)"
//! ```

caiaphas::grammar!(mod arithmetic = "examples/grammars/calculator.ebnf");

use std::{env, process::ExitCode};

use arithmetic::{
    Additive, Calculation, Factor, Multiplicative, Product, Sum,
};

fn main() -> ExitCode {
    let input = env::args().skip(1).collect::<Vec<_>>().join(" ");
    match input.parse::<Calculation>() {
        Ok(calculation) => {
            println!("{}", sum(&calculation.sum));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn sum(sum: &Sum) -> f64 {
    sum.addition
        .iter()
        .fold(product(&sum.product), |value, addition| {
            let operand = product(&addition.product);
            match addition.additive {
                Additive::Plus(_) => value + operand,
                Additive::Minus(_) => value - operand,
            }
        })
}

fn product(product: &Product) -> f64 {
    product.multiplication.iter().fold(
        factor(&product.factor),
        |value, multiplication| {
            let operand = factor(&multiplication.factor);
            match multiplication.multiplicative {
                Multiplicative::Times(_) => value * operand,
                Multiplicative::Over(_) => value / operand,
            }
        },
    )
}

fn factor(factor: &Factor) -> f64 {
    match factor {
        Factor::Number(number) => number
            .digits
            .text()
            .parse()
            .expect("the grammar only matches numbers Rust reads"),
        Factor::Group(group) => sum(&group.sum),
    }
}
