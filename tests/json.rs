//! Parses JSON with the types generated from its grammar.

caiaphas::grammar!(mod json = "tests/grammars/json.ebnf");

use json::{Json, Value};

#[test]
fn parses_a_document_into_the_types_of_its_rules() {
    let document = Json::parse(
        r#" {"name": "caiaphas", "tags": ["ebnf", 1.5e3, true]} "#,
    )
    .unwrap();
    let Value::Object(object) = &document.value else {
        panic!("{document:?}");
    };
    assert_eq!(object.member.len(), 2);
    assert_eq!(object.member[0].string.text(), r#""name""#);
    let Value::Array(tags) = &object.member[1].value else {
        panic!("{object:?}");
    };
    let texts: Vec<&str> = tags.value.iter().map(Value::text).collect();
    assert_eq!(texts, [r#""ebnf""#, "1.5e3", "true"]);
    assert_eq!(tags.value[2], Value::True);
    let Value::Number(number) = &tags.value[1] else {
        panic!("{tags:?}");
    };
    assert_eq!(number.integer.text(), "1");
    assert_eq!(number.exponent.as_ref().map(|e| e.text()), Some("e3"));
}

#[test]
fn says_where_an_input_stops_matching() {
    let error = Json::parse("{\"a\": 1]").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected `,`, `}`, a digit, an exponent or a fraction, found `]` at \
         line 1, column 8"
    );
}

#[test]
fn keeps_where_each_rule_matched() {
    let document = Json::parse("[\n  1,\n  22\n]").unwrap();
    let Value::Array(array) = &document.value else {
        panic!("{document:?}");
    };
    let Value::Number(second) = &array.value[1] else {
        panic!("{array:?}");
    };
    assert_eq!(second.span().as_str(), "22");
    assert_eq!((second.span().line(), second.span().column()), (3, 3));
}
