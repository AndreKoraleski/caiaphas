//! Runs the `caiaphas` command on grammars written to files.

use std::{
    env, fs,
    path::PathBuf,
    process::{self, Command, Output},
};

/// Writes `source` to a file of its own named after `name`, and returns where.
fn grammar(name: &str, source: &str) -> PathBuf {
    let path = env::temp_dir()
        .join(format!("caiaphas-{}-{name}.ebnf", process::id()));
    fs::write(&path, source).unwrap();
    path
}

fn caiaphas(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_caiaphas"))
        .args(arguments)
        .output()
        .unwrap()
}

fn printed(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn prints_suggestions_without_failing() {
    let path = grammar(
        "suggestions",
        "number = digit, {digit}; digit = ? U+0030..U+0039 ?;",
    );
    let output = caiaphas(&["check", path.to_str().unwrap()]);
    assert!(output.status.success());
    let printed = printed(&output);
    assert!(printed.starts_with("suggestion: `digit`"), "{printed}");
}

#[test]
fn fails_at_a_problem_that_would_stop_the_build() {
    let path = grammar("problem", "s = a;");
    let output = caiaphas(&["check", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    let printed = printed(&output);
    assert!(
        printed.starts_with("error: `a` is not defined"),
        "{printed}"
    );
}

#[test]
fn passes_a_grammar_without_findings() {
    let path = grammar("clean", "s = 'x';");
    let output = caiaphas(&["check", path.to_str().unwrap()]);
    assert!(output.status.success());
    assert_eq!(printed(&output), "");
}

#[test]
fn fails_at_a_grammar_it_cannot_read() {
    let output = caiaphas(&["check", "no/such/grammar.ebnf"]);
    assert_eq!(output.status.code(), Some(1));
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.starts_with("error: cannot read"), "{errors}");
}

#[test]
fn tells_how_to_call_it() {
    for arguments in [&[][..], &["check"], &["build"]] {
        let output = caiaphas(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
    assert!(printed(&caiaphas(&["--help"])).contains("usage:"));
    assert!(printed(&caiaphas(&["--version"])).starts_with("caiaphas "));
}
