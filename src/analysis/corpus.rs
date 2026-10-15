//! A corpus of realistic grammars, with every finding the analysis should
//! report about each, to catch checks that report too much or too little.

use crate::{analysis::tests::findings, span::Span};

/// Returns each finding about `source` as the text it points at, its message
/// and the text each of its labels points at.
fn summary(source: &str) -> Vec<String> {
    let text = |span: Span| &source[span.start..span.end];
    findings(source)
        .into_iter()
        .map(|finding| {
            let labels: Vec<&str> = finding
                .labels
                .iter()
                .map(|label| text(label.span))
                .collect();
            format!("{}: {finding} {labels:?}", text(finding.span))
        })
        .collect()
}

#[test]
fn accepts_json_where_one_character_decides_each_choice() {
    let source = "
        json = ws, value, ws;
        value = object | array | string | number | 'true' | 'false' | 'null';
        object = '{', ws, [member, ws, {',', ws, member, ws}], '}';
        member = string, ws, ':', ws, value;
        array = '[', ws, [value, ws, {',', ws, value, ws}], ']';
        string = '\"', {character}, '\"';
        character = ? U+0020..U+0021 ? | ? U+0023..U+005B ?
                  | ? U+005D..U+10FFFF ? | '\\', escape;
        escape = '\"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't'
               | 'u', 4 * hex;
        hex = ? U+0030..U+0039 ? | ? U+0041..U+0046 ? | ? U+0061..U+0066 ?;
        number = ['-'], integer, [fraction], [exponent];
        integer = '0' | ? U+0031..U+0039 ?, {digit};
        digit = ? U+0030..U+0039 ?;
        fraction = '.', digit, {digit};
        exponent = ('e' | 'E'), ['+' | '-'], digit, {digit};
        ws = {' ' | ? U+0009 ? | ? U+000A ? | ? U+000D ?};
    ";
    assert_eq!(summary(source), Vec::<String>::new());
}

#[test]
fn accepts_arithmetic_with_repetition_for_each_operator() {
    let source = "
        grammar = expression;
        expression = term, {('+' | '-'), term};
        term = factor, {('*' | '/'), factor};
        factor = number | '(', expression, ')';
        number = digit, {digit};
        digit = ? U+0030..U+0039 ?;
    ";
    assert_eq!(summary(source), Vec::<String>::new());
}

#[test]
fn accepts_lambda_calculus_that_expects_bodies_to_extend_right() {
    let source = r"
        program     = term;
        term        = abstraction | application;
        abstraction = lambda, variable, '.', term;
        lambda      = '\' | ? U+03BB ?;
        (* expect repeated-follow-conflict: the body of an abstraction
           extends as far right as it can *)
        application = atom, {' ', argument};
        argument    = atom | abstraction;
        atom        = variable | '(', term, ')';
        variable    = letter, {letter | digit};
        letter      = ? U+0061..U+007A ?;
        digit       = ? U+0030..U+0039 ?;
    ";
    assert_eq!(summary(source), Vec::<String>::new());
}

#[test]
fn finds_the_dangling_else() {
    let source = "
        program = statement;
        statement = 'if', 'c', 'then', statement, ['else', statement] | 'x';
    ";
    assert_eq!(
        summary(source),
        [
            "['else', statement]: this optional sequence and what comes after \
             it can both start with 'e', so a parser cannot tell whether to \
             take it []"
        ]
    );
}

#[test]
fn finds_whitespace_that_two_rules_can_both_read() {
    let source = "
        json = value;
        value = '{', ws, [members], ws, '}' | 'n';
        members = value, {',', value};
        ws = {' '};
    ";
    assert_eq!(
        summary(source),
        [
            "{' '}: this repeated sequence and what comes after it can both \
             start with ' ', so a parser cannot tell whether to repeat it \
             again []"
        ]
    );
}

#[test]
fn finds_every_problem_with_an_ambiguous_expression_grammar() {
    let source = "
        grammar = expression;
        expression = expression, '+', expression
                   | expression, '*', expression
                   | 'n';
    ";
    let rule = "expression = expression, '+', expression
                   | expression, '*', expression
                   | 'n';";
    let left_recursion = format!(
        "expression: `expression` can reach itself before reading anything, \
         so its parser would never end [{rule:?}]"
    );
    assert_eq!(
        summary(source),
        [
            left_recursion.clone(),
            "expression, '+', expression: this alternative starts and ends \
             with `expression`, so it can group either way []"
                .to_owned(),
            left_recursion,
            "expression, '*', expression: this alternative starts and ends \
             with `expression`, so it can group either way []"
                .to_owned(),
            "expression, '*', expression: another branch can start with every \
             character this one can [\"expression, '+', expression\"]"
                .to_owned(),
            "'n': another branch can start with every character this one can \
             [\"expression, '+', expression\"]"
                .to_owned(),
        ]
    );
}

#[test]
fn finds_left_recursion_in_a_textbook_expression_grammar() {
    let source = "
        grammar = expression;
        expression = expression, '+', term | term;
        term = 'n';
    ";
    assert_eq!(
        summary(source),
        [
            "expression: `expression` can reach itself before reading \
             anything, so its parser would never end \
             [\"expression = expression, '+', term | term;\"]",
            "term: another branch can start with every character this one can \
             [\"expression, '+', term\"]",
        ]
    );
}
