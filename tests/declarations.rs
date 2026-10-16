//! Parses a grammar that uses every kind of type and expression caiaphas
//! generates.

caiaphas::grammar!(mod declarations = "tests/grammars/declarations.ebnf");

use declarations::{Colour, Expression, Literal, Marker, Program, Type};

#[test]
fn builds_structs_enums_and_boxes() {
    let program =
        Program::parse("let answer: int = ((42));\nlet flag: bool;").unwrap();
    assert_eq!(program.declaration.len(), 2);
    let answer = &program.declaration[0];
    assert_eq!(answer.name.text(), "answer");
    assert_eq!(answer.r#type, Type::Int);
    let Some(Expression::Group(outer)) = &answer.expression else {
        panic!("{answer:?}");
    };
    let Expression::Group(inner) = &*outer.expression else {
        panic!("{outer:?}");
    };
    let Expression::Literal(Literal::Number(number)) = &*inner.expression
    else {
        panic!("{inner:?}");
    };
    assert_eq!(number.text(), "42");
    let flag = &program.declaration[1];
    assert_eq!(flag.r#type, Type::Bool);
    assert_eq!(flag.r#type.text(), "bool");
    assert!(flag.expression.is_none());
}

#[test]
fn leaves_out_what_an_exception_matches() {
    assert!(Program::parse("let lettuce: bool;").is_ok());
    assert!(Program::parse("let let: bool;").is_err());
}

#[test]
fn matches_a_factor_its_number_of_times() {
    assert_eq!(Colour::parse("#00ff7f").unwrap().hex.len(), 6);
    assert!(Colour::parse("#00ff7").is_err());
}

#[test]
fn moves_a_field_whose_name_is_taken() {
    let marker = Marker::parse("@1-9").unwrap();
    assert_eq!(marker.span_.text(), "1-9");
    assert_eq!(marker.span().as_str(), "@1-9");
}
