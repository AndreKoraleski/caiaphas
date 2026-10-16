//! The `caiaphas` command checks ISO/IEC 14977 EBNF grammars as the `grammar!`
//! macro does, without building anything, and also prints the suggestions that
//! would make their parsers faster. It also explains each kind of finding.

use std::{env, fs, process::ExitCode};

const USAGE: &str = "\
usage: caiaphas check <grammar>...
       caiaphas explain <code>";

const HELP: &str = "\
caiaphas checks ISO/IEC 14977 EBNF grammars as the grammar! macro does, and
prints every problem and every suggestion that would make a parser faster.
caiaphas explain tells what a kind of finding means, by the code a finding
gives, such as tied-branches.

usage: caiaphas check <grammar>...
       caiaphas explain <code>

It exits with 1 when a grammar cannot be read, has a problem that would stop
the build, or a code names no finding, and with 2 when it is called wrongly.";

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
        Some("explain") => match (arguments.next(), arguments.next()) {
            (Some(code), None) => explain(&code),
            _ => misuse(),
        },
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

/// Prints what the finding with `code` means. Fails when no finding has it.
fn explain(code: &str) -> ExitCode {
    if let Some(explanation) = caiaphas_generator::explain(code) {
        println!("{explanation}");
        ExitCode::SUCCESS
    } else {
        eprintln!("error: `{code}` is not the code of any finding");
        ExitCode::FAILURE
    }
}

/// Checks the grammar in each of `paths` and prints what it finds. Fails when
/// a grammar cannot be read or has a problem that would stop the build.
fn check(paths: &[String]) -> ExitCode {
    let mut failed = false;
    let mut found = false;
    for path in paths {
        match fs::read_to_string(path) {
            Ok(source) => {
                let report = caiaphas_generator::check(&source, path);
                for finding in &report.findings {
                    println!("{finding}\n");
                }
                found |= !report.findings.is_empty();
                failed |= report.stops_build;
            }
            Err(error) => {
                eprintln!("error: cannot read `{path}`: {error}");
                failed = true;
            }
        }
    }
    if found {
        println!(
            "`caiaphas explain <code>` tells more about a finding, such as \
             `caiaphas explain left-recursion`."
        );
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
