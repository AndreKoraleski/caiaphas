//! The `caiaphas` command checks ISO/IEC 14977 EBNF grammars as the `grammar!`
//! macro does, without building anything, and prints the suggestions that
//! would make their parsers faster too. It also shows the types a grammar's
//! module would hold, runs a grammar on an input as its parser would, and
//! explains each kind of finding.

#[cfg(test)]
mod differential;
mod interpret;
mod parse;

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use parse::Parse;

const USAGE: &str = "\
usage: caiaphas check <grammar>...
       caiaphas types <grammar>
       caiaphas parse <grammar> [<input>...] [--rule <name>] [--expect-fail]
       caiaphas explain [<code>]";

const HELP: &str = "\
caiaphas check prints every finding the grammar! macro would report about each
ISO/IEC 14977 EBNF grammar, suggestions included. caiaphas types prints the
Rust types the macro would generate for a grammar, each after its rule.
caiaphas parse runs a grammar on an input, from a file, each file of a
directory or standard input, as its generated parser would, and prints the
tree of the match or where it stops matching. It goes on past each error that
a rule written with (* recover *) skips, and prints every error with the tree
of what matches. Given several inputs, it tells whether each matches instead
of its tree.
--rule parses with another rule than the first start rule, and --expect-fail
checks that no input matches, for samples a language must reject.
caiaphas explain tells what a finding means, given its code, such as
tied-branches, and lists every code without one.

usage: caiaphas check <grammar>...
       caiaphas types <grammar>
       caiaphas parse <grammar> [<input>...] [--rule <name>] [--expect-fail]
       caiaphas explain [<code>]

It exits with 1 when a grammar cannot be read, has a problem that would stop
the build, an input does not match, or matches with --expect-fail, or a code
names no finding, and with 2 when it is called wrongly.";

fn main() -> ExitCode {
    // A path need not be text, so the arguments are read as the system gives
    // them.
    let mut arguments = env::args_os().skip(1);
    let command = arguments.next();
    let arguments: Vec<_> = arguments.collect();
    // `--help` after a command asks for help, not for a file of that name.
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let mut arguments = arguments.into_iter();
    match command.as_deref().and_then(OsStr::to_str) {
        Some("check") => {
            let paths: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
            if paths.is_empty() {
                return misuse();
            }
            check(&paths)
        }
        Some("types") => match (arguments.next(), arguments.next()) {
            (Some(path), None) => types(&PathBuf::from(path)),
            _ => misuse(),
        },
        Some("parse") => match Parse::read(arguments) {
            Some(parse) => parse.run(),
            None => misuse(),
        },
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
                found |=
                    report.findings.iter().any(|finding| has_code(finding));
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

/// Returns whether `finding` has a code, as `warning[tied-branches]` does,
/// that `caiaphas explain` tells more about.
fn has_code(finding: &str) -> bool {
    finding
        .split_once(": ")
        .is_some_and(|(kind, _)| kind.ends_with(']'))
}

/// Prints the types the grammar in the file at `path` gives. Fails when it
/// cannot be read or has a problem that would stop the build, and prints the
/// problems instead.
fn types(path: &Path) -> ExitCode {
    let source = match read(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let name = path.to_string_lossy();
    let options = caiaphas_generator::Options::default();
    if let Ok(module) = caiaphas_generator::generate(&source, &name, options) {
        print!("{}", module.types);
        return ExitCode::SUCCESS;
    }
    // The problems read as `caiaphas check` prints them.
    for finding in caiaphas_generator::check(&source, &name).findings {
        println!("{finding}\n");
    }
    ExitCode::FAILURE
}

/// Returns the text in the file at `path`, or why it cannot be read.
fn read(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        return Err(format!(
            "`{}` is a directory, not a file",
            path.display()
        ));
    }
    fs::read_to_string(path)
        .map_err(|error| format!("cannot read `{}`: {error}", path.display()))
}
