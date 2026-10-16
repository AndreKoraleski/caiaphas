use super::*;
use crate::dictionary::tests::build_valid;

fn rule(grammar: &Grammar, position: usize) -> &Expression {
    &grammar.syntax_rules[position].expression
}

#[test]
fn finds_rules_that_can_match_nothing() {
    let grammar = build_valid("a = b, ['x']; b = {'y'} | c; c = 'z';");
    let facts = Facts::new(&grammar);
    let nullable: Vec<bool> = (0..3)
        .map(|position| facts.is_nullable(rule(&grammar, position)))
        .collect();
    assert_eq!(nullable, [true, true, false]);
}

#[test]
fn finds_rules_that_can_finish_matching() {
    let grammar =
        build_valid("s = a | b | c; a = 'x', a; b = a | 'y'; c = [c], 'z';");
    let facts = Facts::new(&grammar);
    let productive: Vec<bool> = (0..4)
        .map(|position| facts.is_productive(rule(&grammar, position)))
        .collect();
    assert_eq!(productive, [true, false, true, true]);
}

#[test]
fn finds_rules_that_only_match_single_characters() {
    let grammar = build_valid(
        "s = a | b | c | d | e; a = 'x' | ? U+0061..U+0063 ?; \
         b = 'xy'; c = a - 'b'; d = a, 'x'; e = 'x' | e;",
    );
    let facts = Facts::new(&grammar);
    let characters: Vec<Option<String>> = (0..6)
        .map(|position| {
            facts
                .single_characters(rule(&grammar, position))
                .map(|set| set.to_string())
        })
        .collect();
    assert_eq!(
        characters,
        [
            None,
            Some("'a'..='c', 'x'".to_owned()),
            None,
            Some("'a', 'c', 'x'".to_owned()),
            None,
            None,
        ]
    );
}

#[test]
fn starts_a_term_with_what_its_exception_leaves() {
    let grammar = build_valid("s = ? U+0061..U+007A ? - ('x' | 'b');");
    let facts = Facts::new(&grammar);
    assert_eq!(
        facts.first(rule(&grammar, 0)).to_string(),
        "'a', 'c'..='w', 'y'..='z'"
    );
}

#[test]
fn finds_what_rules_start_with_through_nullable_parts() {
    let grammar = build_valid("a = ['x'], b, 'w'; b = {'y'}, 'z';");
    let facts = Facts::new(&grammar);
    assert_eq!(facts.first(rule(&grammar, 0)).to_string(), "'x'..='z'");
    assert_eq!(facts.first(rule(&grammar, 1)).to_string(), "'y'..='z'");
}

#[test]
fn finds_what_matches_can_hold_after_their_first_character() {
    let grammar = build_valid(
        "s = 'xyz' | a, b | {'q'}; a = ? U+0061..U+0063 ?; b = 'w', 'v';",
    );
    let facts = Facts::new(&grammar);
    assert_eq!(
        facts.inner(rule(&grammar, 0)).to_string(),
        "'q', 'v'..='w', 'y'..='z'"
    );
    assert_eq!(facts.inner(rule(&grammar, 1)).to_string(), "");
    assert_eq!(facts.inner(rule(&grammar, 2)).to_string(), "'v'");
}

#[test]
fn finds_what_follows_each_rule() {
    let grammar = build_valid("a = b, 'x' | c; b = 'y'; c = b, [b], 'z';");
    let facts = Facts::new(&grammar);
    let follow = |position| facts.follow(RuleId(position));
    assert_eq!(follow(0).characters.to_string(), "");
    assert_eq!(follow(1).characters.to_string(), "'x'..='z'");
    assert_eq!(follow(2).characters.to_string(), "");
    let ends: Vec<bool> = (0..3)
        .map(|position| follow(position).end_of_input)
        .collect();
    assert_eq!(ends, [true, false, true]);
}

#[test]
fn finds_the_meta_identifiers_a_rule_starts_with() {
    let grammar = build_valid("a = [b], c, 'x', [b], c; b = 'y'; c = 'z';");
    let facts = Facts::new(&grammar);
    let names = |found: Vec<(RuleId, Span)>| -> Vec<usize> {
        found
            .into_iter()
            .map(|(RuleId(position), _)| position)
            .collect()
    };
    assert_eq!(
        names(facts.first_meta_identifiers(rule(&grammar, 0))),
        [1, 2]
    );
}

#[test]
fn walks_every_expression_with_what_follows_it() {
    let grammar = build_valid("a = 'x' - 'u', {'y'}, 'z', {'w'};");
    let facts = Facts::new(&grammar);
    let mut seen = Vec::new();
    facts.walk_rule(RuleId(0), &mut |expression, after| {
        if let ExpressionKind::TerminalString(string) = &expression.kind {
            let end = if after.end_of_input { " end" } else { "" };
            seen.push(format!("{string}:{}{end}", after.characters));
        }
    });
    seen.sort();
    assert_eq!(
        seen,
        [
            "u:'y'..='z' end",
            "w:'w' end",
            "x:'y'..='z'",
            "y:'y'..='z'",
            "z:'w' end"
        ]
    );
}
