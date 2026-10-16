//! Parses small grammars that each show one thing about how a generated parser
//! matches.

caiaphas::grammar!(mod list = "tests/grammars/list.ebnf");
caiaphas::grammar!(mod words = "tests/grammars/words.ebnf");
caiaphas::grammar!(mod exclamations = "tests/grammars/exclamations.ebnf");
caiaphas::grammar!(mod marked = "tests/grammars/marked.ebnf");
caiaphas::grammar!(mod items = "tests/grammars/items.ebnf");
caiaphas::grammar!(mod hidden = "tests/grammars/hidden.ebnf");
caiaphas::grammar!(mod going_back = "tests/grammars/going_back.ebnf");
caiaphas::grammar!(mod phrase = "tests/grammars/phrase.ebnf");
caiaphas::grammar!(mod shared = "tests/grammars/shared.ebnf");
caiaphas::grammar!(mod letter_x = "tests/grammars/letter_x.ebnf");
caiaphas::grammar!(mod starts = "tests/grammars/starts.ebnf");
caiaphas::grammar!(mod optimized = "tests/grammars/starts.ebnf", optimize);

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
fn takes_the_first_branch_after_which_the_rest_matches_where_the_rule_asks() {
    fn texts(input: &str) -> Vec<String> {
        let parsed = items::Items::parse(input).unwrap();
        parsed
            .item
            .iter()
            .map(|item| item.text().to_owned())
            .collect()
    }
    assert_eq!(texts("a;ab"), ["a", "ab"]);
    assert_eq!(texts("abb;ab"), ["abb", "ab"]);
    let error = items::Items::parse("ac").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected `;`, `b` or the end of the input, found `c` at line 1, \
         column 2"
    );
}

#[test]
fn leaves_what_the_rest_of_the_input_needs() {
    let parse = |input| phrase::Phrase::parse(input).map(|parsed| parsed.word);
    assert!(matches!(parse("iffy"), Ok(phrase::Word::Keyword(_))));
    let Ok(phrase::Word::Name(name)) = parse("affy") else {
        panic!("a name matches `af`");
    };
    assert_eq!(name.text(), "af");
    assert_eq!(
        parse("if").unwrap_err().to_string(),
        "expected `fy`, found the end of the input at line 1, column 3"
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

#[test]
fn matches_the_same_when_optimized() {
    for input in ["zx", "zzy", "w", "zq", "z", ""] {
        let plain = starts::Starts::parse(input)
            .map(|parsed| parsed.item.map(|item| item.text().to_owned()));
        let optimized = optimized::Starts::parse(input)
            .map(|parsed| parsed.item.map(|item| item.text().to_owned()));
        assert_eq!(plain, optimized, "{input:?}");
    }
}

caiaphas::grammar!(mod inline { "inline = 'x', {'y'};" });

#[test]
fn parses_a_grammar_written_in_the_macro() {
    assert_eq!(inline::Inline::parse("xyy").unwrap().text(), "xyy");
    assert!(inline::Inline::parse("y").is_err());
}

caiaphas::grammar!(mod split = "tests/grammars/split.ebnf");
caiaphas::grammar!(mod twice = "tests/grammars/twice.ebnf");

/// Returns every text of at most `length` characters of `alphabet`.
fn every_text(alphabet: &str, length: usize) -> Vec<String> {
    let mut texts = vec![String::new()];
    let mut longest = texts.clone();
    for _ in 0..length {
        longest = longest
            .iter()
            .flat_map(|text| {
                alphabet.chars().map(move |c| format!("{text}{c}"))
            })
            .collect();
        texts.extend(longest.iter().cloned());
    }
    texts
}

#[test]
fn matches_exactly_the_texts_of_its_grammar() {
    let letters = |text: &str| {
        !text.is_empty() && text.chars().all(|c| c.is_ascii_lowercase())
    };
    for text in every_text("ab;", 6) {
        let item = |item: &str| {
            item.strip_prefix('a')
                .is_some_and(|rest| rest.chars().all(|c| c == 'b'))
        };
        let matches = text.split(';').all(item);
        assert_eq!(items::Items::parse(&text).is_ok(), matches, "{text:?}");
    }
    for text in every_text("ify", 6) {
        let matches = text.strip_suffix("fy").is_some_and(letters);
        assert_eq!(phrase::Phrase::parse(&text).is_ok(), matches, "{text:?}");
    }
    for text in every_text("ifx", 6) {
        let matches = text.match_indices('f').any(|(index, _)| {
            let (name, other) = (&text[..index], &text[index + 1..]);
            letters(name) && name != "if" && letters(other)
        });
        assert_eq!(split::Split::parse(&text).is_ok(), matches, "{text:?}");
    }
    for text in every_text("ab", 7) {
        let matches = text.starts_with('a')
            && text.ends_with('b')
            && text.matches('a').count() == 2;
        assert_eq!(twice::Twice::parse(&text).is_ok(), matches, "{text:?}");
    }
    for text in every_text("zx", 7) {
        let maybe = text == "z" || text == "zxz";
        assert_eq!(going_back::Maybe::parse(&text).is_ok(), maybe, "{text:?}");
        let several = text.strip_suffix('z').is_some_and(|rounds| {
            rounds.len() % 2 == 0
                && rounds.as_bytes().chunks(2).all(|round| round == b"zx")
        });
        assert_eq!(
            going_back::Several::parse(&text).is_ok(),
            several,
            "{text:?}"
        );
    }
}

#[test]
fn gives_each_part_the_most_text_the_rest_leaves() {
    let parsed = split::Split::parse("iffx").unwrap();
    let names: Vec<&str> = parsed.name.iter().map(split::Name::text).collect();
    assert_eq!(names, ["i", "fx"]);
    let parsed = twice::Twice::parse("abab").unwrap();
    let runs: Vec<&str> = parsed.run.iter().map(twice::Run::text).collect();
    assert_eq!(runs, ["ab", "a"]);
}

caiaphas::grammar!(mod greedy = "tests/grammars/greedy.ebnf");

#[test]
fn takes_each_way_as_it_comes_in_a_greedy_rule() {
    let parse = |input| greedy::Phrase::parse(input).map(|parsed| parsed.word);
    assert!(matches!(parse("iffy"), Ok(greedy::Word::Keyword(_))));
    assert_eq!(
        parse("affy").unwrap_err().to_string(),
        "expected `fy`, found the end of the input at line 1, column 5"
    );
    let sentence = greedy::Sentence::parse("iff;").unwrap();
    assert!(matches!(sentence.pair.word, greedy::Word::Keyword(_)));
    assert!(greedy::Sentence::parse("aff;").is_err());
}
