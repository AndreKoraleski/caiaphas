//! Parses real formats with grammars written from their specifications.

use std::{fs, path::Path};

caiaphas::grammar!(mod iso14977 = "tests/grammars/iso14977.ebnf");

/// Returns the path and text of every grammar file in `directory`, named from
/// the root of the crate.
fn grammar_files(directory: &str) -> Vec<(String, String)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(directory);
    let mut files: Vec<(String, String)> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ebnf"))
        .map(|path| {
            let text = fs::read_to_string(&path).unwrap();
            (path.display().to_string(), text)
        })
        .collect();
    files.sort();
    files
}

#[test]
fn reads_every_grammar_of_the_crate_as_iso_14977() {
    let files = [
        grammar_files("tests/grammars"),
        grammar_files("examples/grammars"),
    ]
    .concat();
    assert!(files.len() > 20);
    for (path, text) in files {
        if let Err(error) = iso14977::Syntax::parse(&text) {
            panic!("{path}: {error}");
        }
    }
}

#[test]
fn reads_the_parts_of_a_syntax_rule() {
    use iso14977::{SyntacticPrimary, Syntax};

    let syntax: Syntax =
        "(* a list *) list = item, {',', item}; item = 3 * ? U+0041 ?;"
            .parse()
            .unwrap();
    let names: Vec<&str> = syntax
        .syntax_rule
        .iter()
        .map(|rule| rule.meta_identifier.text())
        .collect();
    assert_eq!(names, ["list", "item"]);
    let item = &syntax.syntax_rule[1].definitions_list.single_definition[0]
        .syntactic_term[0]
        .syntactic_factor;
    assert_eq!(item.integer, Some(3));
    assert!(matches!(
        item.syntactic_primary,
        Some(SyntacticPrimary::SpecialSequence(_))
    ));
}

#[test]
fn tells_where_a_grammar_stops_being_iso_14977() {
    let error = iso14977::Syntax::parse("a = 'x' b;").unwrap_err();
    assert_eq!(error.column(), 9);
}
