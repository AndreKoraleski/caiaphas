//! Runs the `caiaphas` command on grammars written to files.

use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    process::{self, Command, Output, Stdio},
};

/// Writes `source` to a file of its own named after `name`, and returns where.
fn grammar(name: &str, source: &str) -> PathBuf {
    written(&format!("{name}.ebnf"), source)
}

/// Writes `text` to an input file of its own named after `name`, and returns
/// where.
fn input(name: &str, text: &str) -> PathBuf {
    written(&format!("{name}.txt"), text)
}

/// Writes `text` to a file of its own named `name`, and returns where.
fn written(name: &str, text: &str) -> PathBuf {
    let path =
        env::temp_dir().join(format!("caiaphas-{}-{name}", process::id()));
    fs::write(&path, text).unwrap();
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
    assert!(
        printed.starts_with("suggestion[single-character-rule]: `digit`"),
        "{printed}"
    );
    assert!(
        printed
            .trim_end()
            .ends_with("`caiaphas explain left-recursion`."),
        "{printed}"
    );
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
    for arguments in [
        &[][..],
        &["check"],
        &["build"],
        &["parse"],
        &["parse", "a", "b", "c"],
        &["parse", "a", "--rule"],
    ] {
        let output = caiaphas(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
    assert!(printed(&caiaphas(&["--help"])).contains("usage:"));
    assert!(printed(&caiaphas(&["--version"])).starts_with("caiaphas "));
}

#[test]
fn explains_a_kind_of_finding() {
    let output = caiaphas(&["explain", "tied-branches"]);
    assert!(output.status.success());
    let printed = printed(&output);
    assert!(
        printed.starts_with("Warning. Two alternatives"),
        "{printed}"
    );
}

#[test]
fn fails_at_a_code_of_no_finding() {
    let output = caiaphas(&["explain", "tied-branch"]);
    assert_eq!(output.status.code(), Some(1));
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(
        errors.trim_end().ends_with("did you mean `tied-branches`?"),
        "{errors}"
    );
    assert_eq!(caiaphas(&["explain", "a", "b"]).status.code(), Some(2));
}

#[test]
fn lists_every_code_with_how_serious_it_is() {
    let output = caiaphas(&["explain"]);
    assert!(output.status.success());
    let printed = printed(&output);
    assert!(printed.starts_with("left-recursion "), "{printed}");
    assert!(printed.contains("tied-branches "), "{printed}");
    assert!(printed.trim_end().ends_with("warning"), "{printed}");
}

#[test]
fn prints_the_types_of_a_grammar() {
    let path = grammar(
        "types",
        "pair = key, '=', [value];\n\
         (* as String *) key = ? /[a-z]+/ ?;\n\
         value = ? /[0-9]+/ ?;",
    );
    let output = caiaphas(&["types", path.to_str().unwrap()]);
    assert!(output.status.success());
    assert_eq!(
        printed(&output),
        "/// pair = key, '=', [value];\n\
         pub struct Pair {\n    pub key: String,\n    \
         pub value: Option<Value>,\n}\n\
         \n\
         /// value = ? /[0-9]+/ ?;\n\
         pub struct Value {}\n"
    );
}

#[test]
fn prints_the_problems_of_a_grammar_without_types() {
    let path = grammar("no-types", "s = a;");
    let output = caiaphas(&["types", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        printed(&output).starts_with("error: `a` is not defined"),
        "{}",
        printed(&output)
    );
}

/// A grammar of pairs, a conversion to a type the command lacks among them.
const PAIRS: &str = "pair = key, '=', value;\n\
                     (* as String *) key = ? /[a-z]+/ ?;\n\
                     (* as Point *) point = ? /[0-9]+/ ?;\n\
                     value = point | key;";

#[test]
fn prints_the_tree_of_an_input() {
    let path = grammar("parse", PAIRS);
    let input = input("parse", "ab=12");
    let output =
        caiaphas(&["parse", path.to_str().unwrap(), input.to_str().unwrap()]);
    assert!(output.status.success());
    assert_eq!(
        printed(&output),
        "pair 1:1 \"ab=12\"\n  key 1:1 \"ab\"\n  value 1:4 \"12\"\n    \
         point 1:4 \"12\"\n"
    );
    let errors = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        errors,
        "note: `Point` is not a type this command has, so a rule that \
         converts to it takes any text here\n"
    );
}

#[test]
fn points_at_where_an_input_stops_matching() {
    let path = grammar("mismatch", PAIRS);
    let input = input("mismatch", "ab=12\n");
    let output =
        caiaphas(&["parse", path.to_str().unwrap(), input.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(
        errors.ends_with(
            "error: expected a digit or the end of the input, found `\\n` at \
             line 1, column 6\n1 | ab=12\n  |      ^\n"
        ),
        "{errors}"
    );
}

#[test]
fn parses_standard_input_from_another_rule() {
    let path = grammar("rule", PAIRS);
    let mut child = Command::new(env!("CARGO_BIN_EXE_caiaphas"))
        .args(["parse", path.to_str().unwrap(), "-", "--rule", "value"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"cd").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(printed(&output), "value 1:1 \"cd\"\n  key 1:1 \"cd\"\n");
}

#[test]
fn fails_at_a_rule_a_parse_cannot_start_from() {
    let path = grammar("no-rule", PAIRS);
    let input = input("no-rule", "12");
    for (rule, error) in [
        ("pairs", "error: the grammar has no rule `pairs`\n"),
        (
            "point",
            "error: `point` has no type of its own, so a parse cannot \
             start from it\n",
        ),
    ] {
        let output = caiaphas(&[
            "parse",
            path.to_str().unwrap(),
            input.to_str().unwrap(),
            "--rule",
            rule,
        ]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(String::from_utf8(output.stderr).unwrap(), error);
    }
}

#[test]
fn prints_the_problems_of_a_grammar_before_parsing() {
    let path = grammar("no-parse", "s = a;");
    let output = caiaphas(&["parse", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        printed(&output).starts_with("error: `a` is not defined"),
        "{}",
        printed(&output)
    );
}
