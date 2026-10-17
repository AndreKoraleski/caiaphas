//! The `caiaphas` command checks ISO/IEC 14977 EBNF grammars as the `grammar!`
//! macro does, without building anything, and prints the suggestions that
//! would make their parsers faster too. It also shows the types a grammar's
//! module would hold, runs a grammar on an input as its parser would, and
//! explains each kind of finding.

#[cfg(test)]
mod differential;
mod interpret;

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
};

use interpret::Program;

const USAGE: &str = "\
usage: caiaphas check <grammar>...
       caiaphas types <grammar>
       caiaphas parse <grammar> [<input>] [--rule <name>]
       caiaphas explain [<code>]";

const HELP: &str = "\
caiaphas check prints every finding the grammar! macro would report about each
ISO/IEC 14977 EBNF grammar, suggestions included. caiaphas types prints the
Rust types the macro would generate for a grammar, each after its rule.
caiaphas parse runs a grammar on an input, from a file or standard input, as
its generated parser would, and prints the tree of the match or where it
stops matching. --rule parses with another rule than the first start rule.
caiaphas explain tells what a finding means, given its code, such as
tied-branches, and lists every code without one.

usage: caiaphas check <grammar>...
       caiaphas types <grammar>
       caiaphas parse <grammar> [<input>] [--rule <name>]
       caiaphas explain [<code>]

It exits with 1 when a grammar cannot be read, has a problem that would stop
the build, an input does not match, or a code names no finding, and with 2
when it is called wrongly.";

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

/// A call of `caiaphas parse`.
struct Parse {
    grammar: PathBuf,
    /// The file of the input, or `None` for standard input.
    input: Option<PathBuf>,
    /// The name of the rule to parse with, if not the first start rule.
    rule: Option<String>,
}

impl Parse {
    /// Returns the call that `arguments` make, if they make one.
    fn read(mut arguments: impl Iterator<Item = OsString>) -> Option<Self> {
        let mut paths = Vec::new();
        let mut rule = None;
        while let Some(argument) = arguments.next() {
            if argument == "--rule" {
                rule = Some(arguments.next()?.to_string_lossy().into_owned());
            } else {
                paths.push(PathBuf::from(argument));
            }
        }
        let mut paths = paths.into_iter();
        let grammar = paths.next()?;
        let input = paths.next().filter(|path| path.as_os_str() != "-");
        if paths.next().is_some() {
            return None;
        }
        Some(Self {
            grammar,
            input,
            rule,
        })
    }

    /// Runs the grammar on the input, and prints the tree of the match or why
    /// it does not match.
    fn run(self) -> ExitCode {
        let source = match read(&self.grammar) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::FAILURE;
            }
        };
        let name = self.grammar.to_string_lossy();
        let plan = match caiaphas_generator::plan::of(&source, &name) {
            Ok(plan) => plan,
            Err(problems) => {
                for problem in problems {
                    println!("{problem}\n");
                }
                return ExitCode::FAILURE;
            }
        };
        let start = match &self.rule {
            None => plan.starts[0],
            Some(rule) => {
                let found = plan
                    .rules
                    .iter()
                    .position(|planned| words(&planned.name) == words(rule));
                let Some(found) = found else {
                    eprintln!("error: the grammar has no rule `{rule}`");
                    return ExitCode::FAILURE;
                };
                // Only a rule with a type has a parse of its own.
                if !plan.rules[found].typed {
                    eprintln!(
                        "error: `{rule}` has no type of its own, so a parse \
                         cannot start from it"
                    );
                    return ExitCode::FAILURE;
                }
                found
            }
        };
        // The input is read last, so a grammar's problems show without waiting
        // for standard input.
        let input =
            match self.input.as_deref().map_or_else(standard_input, read) {
                Ok(input) => input,
                Err(error) => {
                    eprintln!("error: {error}");
                    return ExitCode::FAILURE;
                }
            };
        let program = Program::new(plan, start);
        for ty in program.unchecked() {
            eprintln!(
                "note: `{ty}` is not a type this command has, so a rule that \
                 converts to it takes any text here"
            );
        }
        // An interpreted rule takes more stack than a generated one, so the
        // parse runs where there is plenty.
        let parsed = thread::Builder::new()
            .stack_size(STACK)
            .spawn(move || {
                program.parse(&input, DEPTH).map_err(|error| (error, input))
            })
            .expect("a thread can start")
            .join()
            .expect("a parse does not panic");
        match parsed {
            Ok(tree) => {
                print!("{tree}");
                ExitCode::SUCCESS
            }
            Err((error, input)) => {
                eprintln!("error: {error}");
                eprintln!("{}", error.excerpt(&input));
                ExitCode::FAILURE
            }
        }
    }
}

/// How many matches of recursive rules a parse follows inside one another, as
/// a generated parser does unless its grammar says otherwise.
const DEPTH: usize = caiaphas::__private::DEPTH;

/// The stack of the thread an interpreted parse runs on.
const STACK: usize = 256 << 20;

/// Returns the words of a rule's name, so names compare however they are
/// spaced.
fn words(name: &str) -> Vec<&str> {
    name.split_whitespace().collect()
}

/// Returns what standard input holds, or why it cannot be read.
fn standard_input() -> Result<String, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map(|_| input)
        .map_err(|error| format!("cannot read the input: {error}"))
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
