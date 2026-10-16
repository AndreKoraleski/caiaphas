//! Prints a table of comma-separated values read from standard input, with its
//! columns lined up:
//!
//! ```text
//! printf 'name,motto\ncaiaphas,"say ""hi"""\n' | cargo run --example csv
//! ```

caiaphas::grammar!(mod csv = "examples/grammars/csv.ebnf");

use std::{
    convert::Infallible,
    io::{self, Read},
    process::ExitCode,
    str::FromStr,
};

use csv::Table;

/// The text of a field, without the quotes around it and with each doubled
/// quote read as one.
#[derive(Debug, Clone, PartialEq)]
struct Cell(String);

impl FromStr for Cell {
    type Err = Infallible;

    fn from_str(field: &str) -> Result<Self, Self::Err> {
        let quoted = field
            .strip_prefix('"')
            .and_then(|field| field.strip_suffix('"'));
        Ok(Self(match quoted {
            Some(quoted) => quoted.replace("\"\"", "\""),
            None => field.to_owned(),
        }))
    }
}

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read the input: {error}");
        return ExitCode::FAILURE;
    }
    let table = match input.parse::<Table>() {
        Ok(table) => table,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let mut widths = Vec::new();
    for record in &table.record {
        for (column, Cell(text)) in record.field.iter().enumerate() {
            let width = text.chars().count();
            match widths.get_mut(column) {
                Some(widest) => *widest = width.max(*widest),
                None => widths.push(width),
            }
        }
    }
    for record in &table.record {
        let cells: Vec<String> = record
            .field
            .iter()
            .zip(&widths)
            .map(|(Cell(text), &width)| format!("{text:width$}"))
            .collect();
        println!("{}", cells.join(" | ").trim_end());
    }
    ExitCode::SUCCESS
}
