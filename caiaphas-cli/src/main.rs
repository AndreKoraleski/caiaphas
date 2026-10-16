//! The `caiaphas` command checks ISO/IEC 14977 EBNF grammars as the `grammar!`
//! macro does, without building anything, and prints the suggestions that
//! would make their parsers faster too. It also explains each kind of finding.

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

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
    // A path need not be text, so the arguments are read as the system gives
    // them.
    let mut arguments = env::args_os().skip(1);
    match arguments.next().as_deref().and_then(OsStr::to_str) {
        Some("check") => {
            let paths: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
            if paths.is_empty() {
                return misuse();
            }
            check(&paths)
        }
        Some("explain") => match (arguments.next(), arguments.next()) {
            (None, _) => list(),
            (Some(code), None) => explain(&code.to_string_lossy()),
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
fn check(paths: &[PathBuf]) -> ExitCode {
    let mut failed = false;
    let mut found = false;
    for path in paths {
        match read(path) {
            Ok(source) => {
                let report = caiaphas_generator::check(
                    &source,
                    &path.to_string_lossy(),
                );
                for finding in &report.findings {
                    println!("{finding}\n");
                }
                found |= !report.findings.is_empty();
                failed |= report.stops_build;
            }
            Err(error) => {
                eprintln!("error: {error}");
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

/// Returns the grammar in the file at `path`, or why it cannot be read.
fn read(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        return Err(format!(
            "`{}` is a directory, not a grammar",
            path.display()
        ));
    }
    fs::read_to_string(path)
        .map_err(|error| format!("cannot read `{}`: {error}", path.display()))
}
