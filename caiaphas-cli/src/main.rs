//! The `caiaphas` command checks ISO/IEC 14977 EBNF grammars as the `grammar!`
//! macro does, without building anything, and prints the suggestions that
//! would make their parsers faster too. It also explains each kind of finding.

use std::{env, fs, process::ExitCode};

const USAGE: &str = "\
usage: caiaphas check <grammar>...
       caiaphas explain [<code>]";

const HELP: &str = "\
caiaphas check prints every finding the grammar! macro would report about each
ISO/IEC 14977 EBNF grammar, suggestions included. caiaphas explain tells what a
finding means, given its code, such as tied-branches, and lists every code
without one.

usage: caiaphas check <grammar>...
       caiaphas explain [<code>]

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
            (None, _) => list(),
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

/// Prints the code of every finding, with how serious it is.
fn list() -> ExitCode {
    let width = caiaphas_generator::codes().map(str::len).max().unwrap_or(0);
    for code in caiaphas_generator::codes() {
        let explanation =
            caiaphas_generator::explain(code).unwrap_or_default();
        let severity = explanation.split('.').next().unwrap_or_default();
        println!("{code:width$}  {}", severity.to_lowercase());
    }
    ExitCode::SUCCESS
}

/// Prints what the finding with `code` means. Fails when no finding has it.
fn explain(code: &str) -> ExitCode {
    if let Some(explanation) = caiaphas_generator::explain(code) {
        println!("{explanation}");
        return ExitCode::SUCCESS;
    }
    eprintln!("error: `{code}` is not the code of any finding");
    match caiaphas_generator::similar_code(code) {
        Some(similar) => eprintln!("help: did you mean `{similar}`?"),
        None => eprintln!("help: `caiaphas explain` lists every code"),
    }
    ExitCode::FAILURE
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
