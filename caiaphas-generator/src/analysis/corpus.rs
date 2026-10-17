//! A corpus of realistic grammars, with every problem the analysis should
//! report about each, to catch checks that report too much or too little.
//! Suggestions are left out, since they are not problems.

use crate::{analysis::tests::findings, diagnostic::Severity, span::Span};

/// Returns each problem found in `source` as the text it points at, its
/// message and the text each of its labels points at.
fn summary(source: &str) -> Vec<String> {
    let text = |span: Span| &source[span.range()];
    findings(source)
        .into_iter()
        .filter(|finding| finding.kind.severity() != Severity::Suggestion)
        .map(|finding| {
            let labels: Vec<&str> = finding
                .labels
                .iter()
                .map(|label| text(label.span))
                .collect();
            format!("{}: {} {labels:?}", text(finding.span), finding.kind)
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
fn accepts_arithmetic_with_a_rule_for_each_operator() {
    let source = "
        grammar = expression;
        expression = term, {additive, term};
        term = factor, {multiplicative, factor};
        factor = number | '(', expression, ')';
        additive = '+' | '-';
        multiplicative = '*' | '/';
        number = digit, {digit};
        digit = ? U+0030..U+0039 ?;
    ";
    assert_eq!(summary(source), Vec::<String>::new());
}

#[test]
fn finds_operators_that_no_field_keeps() {
    let source = "
        grammar = expression;
        expression = term, {('+' | '-'), term};
        term = factor, {('*' | '/'), factor};
        factor = number | '(', expression, ')';
        number = digit, {digit};
        digit = ? U+0030..U+0039 ?;
    ";
    assert_eq!(
        summary(source),
        [
            "('+' | '-'): nothing in the fields of `expression` tells which \
             of these alternatives matched [\"'+'\", \"'-'\"]",
            "('*' | '/'): nothing in the fields of `term` tells which of \
             these alternatives matched [\"'*'\", \"'/'\"]",
        ]
    );
}

#[test]
fn accepts_lambda_calculus_whose_bodies_extend_right() {
    let source = r"
        program     = term;
        term        = abstraction | application;
        abstraction = lambda, variable, '.', term;
        lambda      = '\' | ? U+03BB ?;
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
fn finds_the_dangling_else_of_a_greedy_rule() {
    let source = "
        program = statement;
        (* greedy *)
        statement = 'if', 'c', 'then', statement, ['else', statement] | 'x';
    ";
    assert_eq!(
        summary(source),
        [
            "['else', statement]: this optional sequence and what comes after \
             it can both start with `e`, so a parser cannot tell whether to \
             take it []"
        ]
    );
}

#[test]
fn finds_whitespace_that_two_rules_can_both_read_in_a_greedy_rule() {
    let source = "
        json = value;
        value = '{', ws, [members], ws, '}' | 'n';
        members = value, {',', value};
        (* greedy *)
        ws = {' '};
    ";
    assert_eq!(
        summary(source),
        [
            "{' '}: this repeated sequence and what comes after it can both \
             start with ` `, so a parser cannot tell whether to repeat it \
             again []"
        ]
    );
}

#[test]
fn finds_left_recursion_in_each_alternative_of_an_expression_grammar_once() {
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
         so its parser would never end [{rule:?}, \"expression\"]"
    );
    assert_eq!(summary(source), [left_recursion]);
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
        ["expression: `expression` can reach itself before reading \
             anything, so its parser would never end \
             [\"expression = expression, '+', term | term;\"]"]
    );
}

#[test]
fn finds_keywords_that_identifiers_also_match() {
    let source = "
        token = keyword | identifier;
        keyword = 'if' | 'else';
        identifier = letter, {letter};
        letter = ? U+0061..U+007A ?;
    ";
    assert_eq!(
        summary(source),
        [
            "identifier: these branches can both match `if`, so the parser \
             takes the one written first [\"keyword\"]"
        ]
    );
    let expected = "
        (* expect tied-branches: a keyword is never an identifier *)
        token = keyword | identifier;
        keyword = 'if' | 'else';
        identifier = letter, {letter};
        letter = ? U+0061..U+007A ?;
    ";
    assert_eq!(summary(expected), Vec::<String>::new());
}
