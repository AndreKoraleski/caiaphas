//! Parses small grammars that each show one thing about how a generated parser
//! matches.

caiaphas::grammar!(mod list = "tests/grammars/list.ebnf");
caiaphas::grammar!(mod words = "tests/grammars/words.ebnf");
caiaphas::grammar!(mod exclamations = "tests/grammars/exclamations.ebnf");
caiaphas::grammar!(mod marked = "tests/grammars/marked.ebnf");
caiaphas::grammar!(mod items = "tests/grammars/items.ebnf");
caiaphas::grammar!(mod hidden = "tests/grammars/hidden.ebnf");
caiaphas::grammar!(mod going_back = "tests/grammars/going_back.ebnf");
caiaphas::grammar!(mod shared = "tests/grammars/shared.ebnf");
caiaphas::grammar!(mod letter_x = "tests/grammars/letter_x.ebnf");

#[test]
fn builds_the_tree_of_an_input() {
    let parsed = list::Numbers::parse("12,(3,4)").unwrap().list;
    assert_eq!(parsed.item.len(), 2);
    assert_eq!(parsed.item[0].text(), "12");
    assert!(parsed.item[0].list.is_none());
    let inner = parsed.item[1].list.as_ref().unwrap();
    let texts: Vec<&str> = inner.item.iter().map(list::Item::text).collect();
    assert_eq!(texts, ["3", "4"]);
}

#[test]
fn reports_a_rule_that_could_not_start_by_its_name() {
    let error = list::Numbers::parse("1,(2,)").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected an item, found `)` at line 1, column 6"
    );
}

#[test]
fn reports_an_input_that_goes_on_past_the_rule_without_hidden_rules() {
    let error = list::Numbers::parse("1)").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected `,` or the end of the input, found `)` at line 1, column 2"
    );
}

#[test]
fn takes_the_branch_that_matches_the_most() {
    let word = words::Word::parse("iffy").unwrap();
    assert!(matches!(word, words::Word::Name(_)), "{word:?}");
}

#[test]
fn takes_the_first_written_of_branches_that_tie() {
    let word = words::Word::parse("if").unwrap();
    assert!(matches!(word, words::Word::Keyword(_)), "{word:?}");
}

#[test]
fn leaves_out_what_an_exception_matches() {
    let parse = exclamations::Exclamation::parse;
    assert!(parse("in!").is_ok());
    assert!(parse("if").is_err());
    assert!(parse("iff").is_ok());
}

#[test]
fn matches_a_factor_its_number_of_times() {
    let parse = exclamations::Exclamation::parse;
    assert!(parse("??").is_ok());
    assert!(parse("?").is_err());
}

#[test]
fn leaves_out_a_rule_that_tried_to_read_nothing() {
    let error = marked::Marked::parse("ay").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected `b` or `x`, found `y` at line 1, column 2"
    );
}

#[test]
fn takes_the_first_branch_that_matches_where_the_rule_asks() {
    let parsed = items::Items::parse("a;ab").unwrap();
    let texts: Vec<&str> = parsed.item.iter().map(items::Item::text).collect();
    assert_eq!(texts, ["a", "ab"]);
    let error = items::Items::parse("abb").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected `;` or the end of the input, found `b` at line 1, column 3"
    );
}

#[test]
fn leaves_out_what_a_hidden_rule_matched() {
    let pair = hidden::Pair::parse("xy").unwrap();
    assert_eq!(pair.letter.text(), "y");
}

#[test]
fn forgets_what_a_sequence_matched_before_it_failed() {
    assert_eq!(going_back::Maybe::parse("z").unwrap().z.len(), 1);
    assert_eq!(going_back::Several::parse("zxz").unwrap().z.len(), 2);
}

#[test]
fn reports_every_rule_that_starts_with_a_remembered_one() {
    let error = shared::Shared::parse("(w").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected an a or a b, found `w` at line 1, column 2"
    );
}

#[test]
fn takes_out_a_character_without_expecting_anything_else() {
    let parse = letter_x::LetterX::parse;
    assert!(parse("ax").is_ok());
    assert_eq!(
        parse("qx").unwrap_err().to_string(),
        "unexpected `q` at line 1, column 1"
    );
    assert_eq!(
        parse("bq").unwrap_err().to_string(),
        "expected `x`, found `q` at line 1, column 2"
    );
}
