//! Parses a grammar with a rule that takes the first match.

caiaphas::grammar!(mod comparison = "tests/grammars/comparison.ebnf");

use comparison::Comparison;

#[test]
fn takes_the_first_spelling_that_matches() {
    let parsed = Comparison::parse("1<=20").unwrap();
    assert_eq!(parsed.operator.text(), "<=");
    assert_eq!(parsed.number[1].text(), "20");
    assert_eq!(Comparison::parse("3<4").unwrap().operator.text(), "<");
}
