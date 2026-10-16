//! The `caiaphas` command checks ISO/IEC 14977 EBNF grammars as the `grammar!`
//! macro does, without building anything, and also prints the suggestions that
//! would make their parsers faster.

use std::{env, fs, process::ExitCode};

const USAGE: &str = "usage: caiaphas check <grammar>...";

const HELP: &str = "\
caiaphas checks ISO/IEC 14977 EBNF grammars as the grammar! macro does, and
prints every problem and every suggestion that would make a parser faster.

usage: caiaphas check <grammar>...

It exits with 1 when a grammar cannot be read, or has a problem that would
stop the build, and with 2 when it is called wrongly.";

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    match arguments.next().as_deref() {
        Some("check") => {
            let paths: Vec<String> = arguments.collect();
            if paths.is_empty() {
                return misuse();
            }
            check(&paths)
        }
        Some("--help" | "-h") => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("caiaphas {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => misuse(),
    }
}

/// Tells how to call the command, for a call it does not understand.
fn misuse() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// Checks the grammar in each of `paths` and prints what it finds. Fails when
/// a grammar cannot be read or has a problem that would stop the build.
fn check(paths: &[String]) -> ExitCode {
    let mut failed = false;
    for path in paths {
        match fs::read_to_string(path) {
            Ok(source) => {
                let report = caiaphas_generator::check(&source, path);
                for finding in &report.findings {
                    println!("{finding}\n");
                }
                failed |= report.stops_build;
            }
            Err(error) => {
                eprintln!("error: cannot read `{path}`: {error}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
