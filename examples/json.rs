//! Prints the outline of a JSON document read from standard input:
//!
//! ```text
//! echo '{"name": "caiaphas", "tags": ["ebnf", 2]}' | cargo run --example json
//! ```

caiaphas::grammar!(mod json = "examples/grammars/json.ebnf");

use std::{
    io::{self, Read},
    process::ExitCode,
};

use json::{Json, Value};

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read the input: {error}");
        return ExitCode::FAILURE;
    }
    match Json::parse(&input) {
        Ok(document) => {
            outline(&document.value, 0);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Prints `value` indented `depth` levels, and each value inside it a level
/// deeper.
fn outline(value: &Value, depth: usize) {
    let indent = "  ".repeat(depth);
    match value {
        Value::Object(object) => {
            for member in &object.member {
                println!("{indent}{}", member.string.text());
                outline(&member.value, depth + 1);
            }
        }
        Value::Array(array) => {
            for (index, item) in array.value.iter().enumerate() {
                println!("{indent}[{index}]");
                outline(item, depth + 1);
            }
        }
        Value::String(string) => println!("{indent}{}", string.text()),
        Value::Number(number) => println!("{indent}{number}"),
        Value::True => println!("{indent}true"),
        Value::False => println!("{indent}false"),
        Value::Null => println!("{indent}null"),
    }
}
