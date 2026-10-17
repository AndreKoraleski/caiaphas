//! Tests that the README shows grammars caiaphas accepts, and what their
//! parsers give.

const README: &str = include_str!("../README.md");

caiaphas::grammar!(mod json = "examples/grammars/json.ebnf");

/// Returns each block of the README written in `language`, without the indent
/// of its fence.
fn blocks(language: &str) -> Vec<String> {
    let fence = format!("```{language}");
    let mut blocks = Vec::new();
    let mut lines = README.lines();
    while let Some(line) = lines.next() {
        if line.trim_start() != fence {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let block: Vec<&str> = lines
            .by_ref()
            .take_while(|line| line.trim() != "```")
            .map(|line| line.get(indent..).unwrap_or_default())
            .collect();
        blocks.push(block.join("\n"));
    }
    blocks
}

#[test]
fn shows_grammars_that_pass_their_checks() {
    let whole = include_str!("../examples/grammars/json.ebnf");
    let blocks = blocks("ebnf");
    assert!(blocks.len() > 3);
    for block in blocks {
        // A grammar cut short is part of the JSON example.
        if block.contains("(* ... *)") {
            for line in block.lines().filter(|line| *line != "(* ... *)") {
                assert!(whole.lines().any(|whole| whole == line), "{line}");
            }
            continue;
        }
        let report = caiaphas_generator::check(&block, "README.md");
        assert!(
            !report.stops_build,
            "{block}\n\n{}",
            report.findings.join("\n")
        );
    }
}

#[test]
fn reads_the_document_of_the_overview() {
    use json::{Json, Value};

    let document: Json = r#"{"name": "caiaphas"}"#.parse().unwrap();
    let Value::Object(object) = &document.value else {
        panic!("the document is an object");
    };
    let [member] = &object.member[..] else {
        panic!("the object has one member");
    };
    let Value::String(value) = &member.value else {
        panic!("the member holds a string");
    };
    assert_eq!(
        format!("{} is {}", member.string, value),
        r#""name" is "caiaphas""#
    );
}

#[test]
fn shows_the_error_a_parser_gives() {
    let input = r#"{"a": 1]"#;
    let error = json::Json::parse(input).unwrap_err();
    let shown = error.to_string();
    assert!(blocks("text").contains(&shown), "{shown}");
    let excerpt = error.excerpt(input);
    assert!(blocks("text").contains(&excerpt), "{excerpt}");
}

caiaphas::grammar!(mod settings {
    r"settings = [line], {newline, [line]};
(* recover *)
line = name, ' = ', value;
name = ? /[a-z]+/ ?;
value = ? /[^\n]+/ ?;
(* hide *) newline = ? U+000A ?;"
});

#[test]
fn recovers_as_the_errors_section_tells() {
    assert!(blocks("ebnf").iter().any(|block| {
        block.contains("(* recover *)\nline = name, ' = ', value;")
    }));
    let recovered =
        settings::Settings::parse_recovering("a = 1\nbroken\nb = 2");
    let names: Vec<&str> = recovered
        .value
        .as_ref()
        .unwrap()
        .line
        .iter()
        .map(|line| line.name.text())
        .collect();
    assert_eq!(names, ["a", "b"]);
    let lines: Vec<usize> =
        recovered.errors.iter().map(caiaphas::Error::line).collect();
    assert_eq!(lines, [2]);
}
