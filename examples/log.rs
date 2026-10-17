//! Prints the lines of a log read from standard input whose level is at least
//! the one given, `INFO` unless another is:
//!
//! ```text
//! printf '2026-10-17 08:30:01 WARN disk full\n' | cargo run --example log
//! ```

caiaphas::grammar!(mod log = "examples/grammars/log.ebnf");

use std::{
    env,
    io::{self, Read},
    process::ExitCode,
    str::FromStr,
};

use log::Log;

/// How serious a line is, from the least to the most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl FromStr for Level {
    type Err = String;

    fn from_str(level: &str) -> Result<Self, Self::Err> {
        match level {
            "TRACE" => Ok(Self::Trace),
            "DEBUG" => Ok(Self::Debug),
            "INFO" => Ok(Self::Info),
            "WARN" => Ok(Self::Warn),
            "ERROR" => Ok(Self::Error),
            _ => Err(format!("`{level}` is not a level")),
        }
    }
}

fn main() -> ExitCode {
    let least = env::args()
        .nth(1)
        .map_or(Ok(Level::Info), |level| level.parse::<Level>());
    let least = match least {
        Ok(least) => least,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read the input: {error}");
        return ExitCode::FAILURE;
    }
    let log = match input.parse::<Log>() {
        Ok(log) => log,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    for line in log.line.iter().filter(|line| line.level >= least) {
        println!("{} {:?}: {}", line.time, line.level, line.message);
    }
    ExitCode::SUCCESS
}
