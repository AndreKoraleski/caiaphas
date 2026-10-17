//! `caiaphas parse` runs a grammar on inputs as its generated parser would. It
//! prints the tree of what matches and every error found, or for several
//! inputs, whether each matches.

use std::{
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
};

use caiaphas::Error;

use crate::{interpret::Program, read};

/// A call of `caiaphas parse`.
pub(crate) struct Parse {
    grammar: PathBuf,
    /// The file of each input, or `None` for standard input.
    inputs: Vec<Option<PathBuf>>,
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
        let mut inputs: Vec<Option<PathBuf>> = paths
            .map(|path| (path.as_os_str() != "-").then_some(path))
            .collect();
        if inputs.is_empty() {
            inputs.push(None);
        }
        // Standard input can only be read once.
        if inputs.iter().filter(|input| input.is_none()).count() > 1 {
            return None;
        }
        Some(Self {
            grammar,
            inputs,
            rule,
        })
    }

    /// Runs the grammar on the inputs, and prints what it finds.
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
                    let typed: Vec<String> = plan
                        .rules
                        .iter()
                        .filter(|planned| planned.typed)
                        .map(|planned| format!("`{}`", planned.name))
                        .collect();
                    eprintln!(
                        "help: a parse can start from {}",
                        typed.join(", ")
                    );
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
        let program = Program::new(plan, start);
        for ty in program.unchecked() {
            eprintln!(
                "note: `{ty}` is not a type this command has, so a rule that \
                 converts to it takes any text here"
            );
        }
        // An interpreted rule takes more stack than a generated one, so the
        // parses run where there is plenty.
        let inputs = files(self.inputs);
        thread::Builder::new()
            .stack_size(STACK)
            .spawn(move || report(program, &inputs))
            .expect("a thread can start")
            .join()
            .expect("a parse does not panic")
    }
}

/// Returns `inputs` with each directory among them replaced by the files in
/// it, in order of their names, so a folder of samples is parsed whole.
fn files(inputs: Vec<Option<PathBuf>>) -> Vec<Option<PathBuf>> {
    let mut files = Vec::new();
    for input in inputs {
        let entries = input
            .as_deref()
            .filter(|path| path.is_dir())
            .and_then(|path| fs::read_dir(path).ok());
        let Some(entries) = entries else {
            files.push(input);
            continue;
        };
        let mut inside: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.is_file())
            .collect();
        inside.sort();
        files.extend(inside.into_iter().map(Some));
    }
    files
}

/// Parses each of `inputs` with `program` and prints what it finds: the tree
/// of what matches for one input, and whether each matches for several, with
/// every error. The inputs are read here, so a grammar's problems show without
/// waiting for standard input.
fn report(program: &'static Program, inputs: &[Option<PathBuf>]) -> ExitCode {
    let several = inputs.len() > 1;
    let mut matching = 0;
    for path in inputs {
        let input = match path.as_deref().map_or_else(standard_input, read) {
            Ok(input) => input,
            Err(error) => {
                eprintln!("error: {error}");
                continue;
            }
        };
        let (tree, errors) = program.parse_recovering(&input, DEPTH);
        // A grammar whose rules recover gives the tree of what still matches
        // with the errors it skipped.
        if let Some(tree) = tree.filter(|_| !several) {
            print!("{tree}");
        }
        for error in &errors {
            point(error, path.as_deref(), &input);
        }
        if errors.is_empty() {
            matching += 1;
            if several {
                let name = path.as_deref().map(Path::display);
                match name {
                    Some(name) => println!("ok {name}"),
                    None => println!("ok standard input"),
                }
            }
        }
    }
    if several {
        println!("{matching} of {} inputs match", inputs.len());
    }
    if matching == inputs.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Prints `error` of `input`, with where it is in the file at `path`, if the
/// input comes from one, and the line it is on.
fn point(error: &Error, path: Option<&Path>, input: &str) {
    eprintln!("error: {error}");
    if let Some(path) = path {
        let gutter = " ".repeat(error.line().to_string().len());
        eprintln!(
            "{gutter}--> {}:{}:{}",
            path.display(),
            error.line(),
            error.column()
        );
    }
    eprintln!("{}", error.excerpt(input));
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
