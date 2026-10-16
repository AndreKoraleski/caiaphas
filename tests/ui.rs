//! Checks the errors the macro reports at compile time, against the output
//! kept next to each case in `tests/ui`.

#[test]
fn reports_problems_in_grammars_as_compile_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
