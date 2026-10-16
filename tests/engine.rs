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
fn reports_an_input_that_goes_on_past_the_rule() {
    let error = list::Numbers::parse("1)").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected `,`, a digit or the end of the input, found `)` at line 1, \
         column 2"
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
        "expected `fy` or a letter, found the end of the input at line 1, \
         column 3"
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
        "expected `fy` or a letter, found the end of the input at line 1, \
         column 5"
    );
    let sentence = greedy::Sentence::parse("iff;").unwrap();
    assert!(matches!(sentence.pair.word, greedy::Word::Keyword(_)));
    assert!(greedy::Sentence::parse("aff;").is_err());
}

caiaphas::grammar!(mod assignment {
    "assignment = name, '=', name; name = letter, {letter};
     (* hide *) letter = ? U+0061..U+007A ?;"
});

#[test]
fn holds_a_rule_held_the_same_number_of_times_in_an_array() {
    let [target, value] = assignment::Assignment::parse("x=y").unwrap().name;
    assert_eq!((target.text(), value.text()), ("x", "y"));
}

caiaphas::grammar!(mod digits {
    "digits = digit, {digit}; digit = ? U+0030..U+0039 ?;"
});

#[test]
fn tells_the_character_of_a_rule_of_one_character() {
    let digits = digits::Digits::parse("42").unwrap();
    let characters: String =
        digits.digit.iter().map(digits::Digit::char).collect();
    assert_eq!(characters, "42");
}

caiaphas::grammar!(mod grouped {
    "sum = group, {'+', group};
     (* inline *) group = '(', number, ')' | number;
     number = digit, {digit};
     (* hide *) digit = ? U+0030..U+0039 ?;"
});

caiaphas::grammar!(mod remembered {
    "pair = both, 'x' | both, 'y'; (* inline *) both = a, b; a = 'a'; b = 'b';"
});

#[test]
fn gives_a_rule_that_holds_an_inline_rule_what_it_holds() {
    let sum = grouped::Sum::parse("(1)+23").unwrap();
    let numbers: Vec<&str> =
        sum.number.iter().map(grouped::Number::text).collect();
    assert_eq!(numbers, ["1", "23"]);
    let pair = remembered::Pair::parse("aby").unwrap();
    assert_eq!((pair.a.text(), pair.b.text()), ("a", "b"));
}

#[test]
fn parses_a_start_rule_from_a_str() {
    let parsed: list::Numbers = "1,2".parse().unwrap();
    assert_eq!(parsed.list.item.len(), 2);
    assert!("1,".parse::<list::Numbers>().is_err());
}

caiaphas::grammar!(mod closed {
    "group = '(', 'x', close; (* hide *) close = ')', ';';"
});

#[test]
fn reports_what_a_hidden_rule_expected_unless_it_can_match_nothing() {
    assert_eq!(
        closed::Group::parse("(x").unwrap_err().to_string(),
        "expected `)`, found the end of the input at line 1, column 3"
    );
}

caiaphas::grammar!(mod settings {
    "settings = setting, {';', setting};
     setting = name, '=', value;
     (* expect tied-branches: a flag is never a word *)
     value = number | flag | word;
     (* as u8 *) number = digit, {digit};
     (* as bool *) flag = 'true' | 'false';
     (* as String *) word = letter, {letter};
     (* as String *) name = letter, {letter};
     (* hide *) digit = ? U+0030..U+0039 ?;
     (* hide *) letter = ? U+0061..U+007A ?;"
});

caiaphas::grammar!(mod price {
    "price = amount, ' ', currency;
     (* as f64 *) amount = digit, {digit}, ['.', digit, {digit}];
     currency = 'EUR' | 'USD';
     (* hide *) digit = ? U+0030..U+0039 ?;"
});

caiaphas::grammar!(mod bytes {
    "pair = byte, byte; (* as u8 *) byte = digit, {digit};
     (* hide *) digit = ? U+0030..U+0039 ?;"
});

#[test]
fn reads_the_value_of_a_converted_rule_from_its_text() {
    use settings::{Settings, Value};
    let parsed: Settings = "port=80;debug=true;mode=fast".parse().unwrap();
    let values: Vec<(&str, Value)> = parsed
        .setting
        .iter()
        .map(|setting| (setting.name.as_str(), setting.value.clone()))
        .collect();
    assert_eq!(
        values,
        [
            ("port", Value::Number(80)),
            ("debug", Value::Flag(true)),
            ("mode", Value::Word("fast".to_owned())),
        ]
    );
    let price = price::Price::parse("12.5 EUR").unwrap();
    assert!((price.amount - 12.5).abs() < f64::EPSILON);
}

#[test]
fn fails_where_a_type_does_not_read_the_text() {
    let error = "port=300".parse::<settings::Settings>().unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected a value, found `300` at line 1, column 6"
    );
    assert_eq!(bytes::Pair::parse("2555").unwrap().byte, [255, 5]);
}

caiaphas::grammar!(mod nested {
    "outer = inner | 'y'; inner = number | word;
     (* as i64 *) number = '1'; word = 'x';"
});

#[test]
fn leaves_out_the_text_of_an_enum_that_holds_one_without_it() {
    use nested::{Inner, Outer};
    assert_eq!(Outer::parse("1").unwrap(), Outer::Inner(Inner::Number(1)));
    assert_eq!(Outer::parse("y").unwrap(), Outer::Y);
}

/// An operator read from its sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Operator {
    Plus,
    Minus,
}

impl std::str::FromStr for Operator {
    type Err = ();

    fn from_str(sign: &str) -> Result<Self, ()> {
        match sign {
            "+" => Ok(Self::Plus),
            "-" => Ok(Self::Minus),
            _ => Err(()),
        }
    }
}

caiaphas::grammar!(mod signs {
    "signs = operator, {operator}, text; string = 'x';
     (* as Operator *) operator = '+' | '-';
     (* as ::std::string::String *) text = string, {string};"
});

#[test]
fn names_the_type_of_a_conversion_as_where_the_macro_is() {
    let signs = signs::Signs::parse("+-xx").unwrap();
    assert_eq!(signs.operator, [Operator::Plus, Operator::Minus]);
    assert_eq!(signs.text, "xx");
}

caiaphas::grammar!(mod leftover {
    "(* greedy *) s = head, c;
     head = [d], 'a', {'x'}, 'x';
     d = c, 'y';
     c = ? U+0061..U+0062 ?;"
});

caiaphas::grammar!(mod leftover_variant {
    "(* greedy *) e = head | k;
     head = [d], 'q', {'x'}, 'x';
     d = c, 'y';
     c = 'q';
     k = 'w';"
});

#[test]
fn leaves_no_node_of_a_rule_that_did_not_match() {
    let parsed = leftover::S::parse("axb").unwrap();
    assert_eq!((parsed.c.text(), parsed.c.span().range()), ("b", 2..3));
    let parsed = leftover_variant::E::parse("qx").unwrap();
    assert!(matches!(parsed, leftover_variant::E::Head(_)));
}

caiaphas::grammar!(mod ends_apart {
    "s = a, 'ab' | a, 'b'; a = {'a'};"
});

caiaphas::grammar!(mod ends_apart_optimized {
    "s = a, 'ab' | a, 'b'; a = {'a'};"
}, optimize);

caiaphas::grammar!(mod duplicates_optimized {
    "(* expect duplicate-alternative: kept *) s = 'x' | 'x';"
}, optimize);

#[test]
fn builds_the_same_nodes_with_the_grammar_rearranged() {
    let plain = ends_apart::S::parse("aab").unwrap();
    let optimized = ends_apart_optimized::S::parse("aab").unwrap();
    assert_eq!(plain.a.text(), "a");
    assert_eq!(optimized.a.text(), plain.a.text());
    assert!(duplicates_optimized::S::parse("x").is_ok());
}

caiaphas::grammar!(mod converted_character {
    "s = {c}, 'x';
     (* hide *) (* as u8 *) c = ? U+0030..U+0039 ? | 'a';"
});

#[test]
fn reads_a_converted_character_with_its_type_wherever_it_is() {
    assert!(converted_character::S::parse("12x").is_ok());
    assert!(converted_character::S::parse("ax").is_err());
}

caiaphas::grammar!(mod statement {
    "statement = name, ' = ', name;
     name = letters - keyword;
     (* hide *) letters = letter, {letter};
     (* hide *) keyword = 'let' | 'if';
     (* hide *) letter = ? U+0061..U+007A ?;"
});

caiaphas::grammar!(mod hidden_ranges {
    "pair = 'x', (tail | single);
     (* hide *) tail = ? U+0061..U+007A ?, '!';
     (* hide *) single = ? U+0030..U+0039 ? | ',';"
});

#[test]
fn tells_where_an_exception_took_a_match_away() {
    let error = statement::Statement::parse("let = x").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected a name, found `let` at line 1, column 1"
    );
}

#[test]
fn calls_a_hidden_rule_of_character_ranges_by_its_name() {
    let error = hidden_ranges::Pair::parse("x?").unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected a single or a tail, found `?` at line 1, column 2"
    );
}

caiaphas::grammar!(mod comment {
    "comment = '/*', body, '*/';
     (* as String *) body = {character} - ({character}, '*/', {character});
     (* hide *) character = ? U+0020..U+007E ?;"
});

#[test]
fn ends_a_comment_at_the_first_end_its_exception_names() {
    assert_eq!(comment::Comment::parse("/* a */").unwrap().body, " a ");
    assert!(comment::Comment::parse("/* a */ b */").is_err());
}
