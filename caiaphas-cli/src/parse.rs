//! `caiaphas parse` runs a grammar on an input as its generated parser would,
//! and prints the tree of what matches and every error found.

use std::{
    ffi::OsString,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
    thread,
};

use crate::{interpret::Program, read};

/// A call of `caiaphas parse`.
pub(crate) struct Parse {
    grammar: PathBuf,
    /// The file of the input, or `None` for standard input.
    input: Option<PathBuf>,
    /// The name of the rule to parse with, if not the first start rule.
    rule: Option<String>,
}

impl Parse {
    /// Returns the call that `arguments` make, if they make one.
    pub(crate) fn read(
        mut arguments: impl Iterator<Item = OsString>,
    ) -> Option<Self> {
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

    /// Runs the grammar on the input, and prints the tree of what matches and
    /// every error found.
    pub(crate) fn run(self) -> ExitCode {
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
        let (tree, errors, input) = thread::Builder::new()
            .stack_size(STACK)
            .spawn(move || {
                let (tree, errors) = program.parse_recovering(&input, DEPTH);
                (tree, errors, input)
            })
            .expect("a thread can start")
            .join()
            .expect("a parse does not panic");
        // A grammar whose rules recover gives the tree of what still matches
        // with the errors it skipped.
        if let Some(tree) = tree {
            print!("{tree}");
        }
        for error in &errors {
            eprintln!("error: {error}");
            eprintln!("{}", error.excerpt(&input));
        }
        if errors.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
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
