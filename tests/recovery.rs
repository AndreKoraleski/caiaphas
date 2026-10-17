//! Parses inputs with errors, going on past each one a rule written with
//! `(* recover *)` can skip.

caiaphas::grammar!(mod settings {
    r"settings = [line], {newline, [line]};
      (* recover *)
      line = name, ws, '=', ws, value;
      (* as String *) name = ? /[a-z_]+/ ?;
      (* as String *) value = ? /[^\n ][^\n]*/ ?;
      (* hide *) ws = {' '};
      (* hide *) newline = ? U+000A ?;"
});

#[test]
fn tells_every_error_and_keeps_the_lines_around_them() {
    let recovered = settings::Settings::parse_recovering(
        "a = 1\nbroken\nb = 2\nc 3\nd = 4",
    );
    let lines: Vec<(&str, &str)> = recovered
        .value
        .as_ref()
        .expect("the lines around the errors match")
        .line
        .iter()
        .map(|line| (line.name.as_str(), line.value.as_str()))
        .collect();
    assert_eq!(lines, [("a", "1"), ("b", "2"), ("d", "4")]);
    let places: Vec<(usize, usize)> = recovered
        .errors
        .iter()
        .map(|error| (error.line(), error.column()))
        .collect();
    assert_eq!(places, [(2, 7), (4, 3)]);
    assert_eq!(
        recovered.errors[0],
        settings::Settings::parse("a = 1\nbroken\nb = 2").unwrap_err()
    );
}

#[test]
fn gives_the_value_and_no_errors_where_the_input_matches() {
    let recovered = settings::Settings::parse_recovering("a = 1\nb = 2");
    assert_eq!(recovered.errors, []);
    assert_eq!(recovered.value.unwrap().line.len(), 2);
}

#[test]
fn gives_no_value_where_nothing_after_an_error_matches() {
    let recovered = settings::Settings::parse_recovering("a = 1\nbroken");
    assert_eq!(recovered.value, None);
    assert_eq!(recovered.errors.len(), 1);
}

caiaphas::grammar!(mod files {
    "files = file, {',', file};
     (* recover *)
     file = name, '.', extension;
     name = character, {character};
     extension = letter, {letter};
     (* hide *) character = letter | '.';
     (* hide *) letter = ? U+0061..U+007A ?;"
});

#[test]
fn recovers_in_a_rule_that_tries_every_way() {
    let recovered = files::Files::parse_recovering("a.b,bad,c.tar.gz");
    let names: Vec<(&str, &str)> = recovered
        .value
        .as_ref()
        .expect("the files around the error match")
        .file
        .iter()
        .map(|file| (file.name.text(), file.extension.text()))
        .collect();
    assert_eq!(names, [("a", "b"), ("c.tar", "gz")]);
    assert_eq!(recovered.errors.len(), 1);
    assert_eq!(recovered.errors[0].column(), 8);
}

caiaphas::grammar!(mod plain {
    "pair = 'x', ',', 'y';"
});

#[test]
fn tells_the_one_error_of_a_grammar_without_rules_that_recover() {
    let recovered = plain::Pair::parse_recovering("x;y");
    assert_eq!(recovered.value, None);
    assert_eq!(recovered.errors, [plain::Pair::parse("x;y").unwrap_err()]);
}

caiaphas::grammar!(mod json {
    r#"json = ws, value, ws;
     value = object | array | number | 'true';
     object = '{', ws, [member, ws, {',', ws, member, ws}], '}';
     (* recover *)
     member = string, ws, ':', ws, value;
     array = '[', ws, [value, ws, {',', ws, value, ws}], ']';
     string = '"', ? /[a-z]*/ ?, '"';
     number = ? /\d+/ ?;
     (* hide *) ws = {' '};"#
});

#[test]
fn tells_an_error_in_the_text_a_recovery_skips() {
    let input = r#"{"a": x, "b": y, "c": 3}"#;
    let recovered = json::Json::parse_recovering(input);
    let columns: Vec<usize> = recovered
        .errors
        .iter()
        .map(caiaphas::Error::column)
        .collect();
    assert_eq!(columns, [7, 15]);
    let json::Value::Object(object) = recovered.value.unwrap().value else {
        panic!("the value is an object");
    };
    let names: Vec<&str> = object
        .member
        .iter()
        .map(|member| member.string.text())
        .collect();
    assert_eq!(names, [r#""c""#]);
}
