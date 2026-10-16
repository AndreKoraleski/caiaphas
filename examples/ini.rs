//! Prints each property of an INI file read from standard input, named after
//! its section:
//!
//! ```text
//! printf '[server]\nport = 8080\n' | cargo run --example ini
//! ```

caiaphas::grammar!(mod ini = "examples/grammars/ini.ebnf");

use std::{
    io::{self, Read},
    process::ExitCode,
};

use ini::{Entry, Ini};

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read the input: {error}");
        return ExitCode::FAILURE;
    }
    let file = match input.parse::<Ini>() {
        Ok(file) => file,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let mut section = None;
    for entry in &file.entry {
        match entry {
            Entry::Section(header) => section = Some(&header.name),
            Entry::Property(property) => {
                let value = property
                    .value
                    .as_ref()
                    .map_or("", |value| value.text().trim_end());
                match section {
                    Some(section) => {
                        println!("{section}.{} = {value}", property.name);
                    }
                    None => println!("{} = {value}", property.name),
                }
            }
            Entry::Comment(_) => {}
        }
    }
    ExitCode::SUCCESS
}
