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
fn reads_many_comments_in_one_pass_each() {
    // A comment's text is a run of any character holding no `*)`, so reading
    // every run to the end of the input would take minutes here.
    let grammar = "(* a rule (* and a note *) *)\ns = 'a';\n".repeat(5_000);
    let syntax = iso14977::Syntax::parse(&grammar).unwrap();
    assert_eq!(syntax.syntax_rule.len(), 5_000);
    // The error of a comment left open is found as quickly.
    let open = format!("{grammar}(* open");
    let error = iso14977::Syntax::parse(&open).unwrap_err();
    assert_eq!((error.line(), error.column()), (10_001, 8));
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

caiaphas::grammar!(mod toml = "tests/grammars/toml.ebnf");

/// A document with every kind of TOML value, after the example of the
/// specification.
const DOCUMENT: &str = r#"# This is a TOML document

title = "TOML \"Example\""

[owner]
name = 'Tom Preston-Werner'
dob = 1979-05-27T07:32:00-08:00

[database]
enabled = true
ports = [ 8000, 8001, 8002 ]
data = [ ["delta", "phi"], [3.14] ]
temp_targets = { cpu = 79.5, case = 72.0 }
limits = [
  0xFF, # in hex
]

[servers]

[servers.alpha]
ip = "10.0.0.1"
role = """
frontend
and "more""""

[[products]]
name = "Hammer"
sku = 738_594_937
weight = -inf
when = 07:32:00.999
"#;

#[test]
fn reads_a_toml_document() {
    use toml::{Toml, Value};

    let parsed: Toml = DOCUMENT.parse().unwrap();
    let values: Vec<(&str, &Value)> = parsed
        .expression
        .iter()
        .filter_map(|expression| expression.key_value.as_ref())
        .map(|pair| (pair.key.text(), &pair.value))
        .collect();
    let keys: Vec<&str> = values.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        keys,
        [
            "title",
            "name",
            "dob",
            "enabled",
            "ports",
            "data",
            "temp_targets",
            "limits",
            "ip",
            "role",
            "name",
            "sku",
            "weight",
            "when"
        ]
    );
    let kinds: Vec<&str> = values
        .iter()
        .map(|(_, value)| match value {
            Value::String(_) => "string",
            Value::Boolean(_) => "boolean",
            Value::Array(_) => "array",
            Value::InlineTable(_) => "inline table",
            Value::DateTime(_) => "date time",
            Value::Float(_) => "float",
            Value::Integer(_) => "integer",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "string",
            "string",
            "date time",
            "boolean",
            "array",
            "array",
            "inline table",
            "array",
            "string",
            "string",
            "string",
            "integer",
            "float",
            "date time"
        ]
    );
    let tables: Vec<&str> = parsed
        .expression
        .iter()
        .filter_map(|expression| expression.table.as_ref())
        .map(toml::Table::text)
        .collect();
    assert_eq!(
        tables,
        [
            "[owner]",
            "[database]",
            "[servers]",
            "[servers.alpha]",
            "[[products]]"
        ]
    );
}

#[test]
fn tells_where_a_toml_document_goes_wrong() {
    let document = DOCUMENT.replace("0xFF", "0x_FF");
    let error = toml::Toml::parse(&document).unwrap_err();
    assert_eq!((error.line(), error.column()), (15, 5));
}
