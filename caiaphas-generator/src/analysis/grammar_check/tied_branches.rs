//! Tied branches are alternatives that can match the same text, such as `'if'`
//! and `identifier` in `word = 'if' | identifier;`. A parser takes the
//! alternative that matches the most, so it only has to pick between two when
//! they tie, and then it takes the one written first.
//!
//! Whether two alternatives can match the same text is told for sure when one
//! of them matches single characters or a few fixed texts. Otherwise short
//! texts each may match are tried on both, so a tie found is always real, but
//! one that needs a longer text can go unreported. Alternatives written the
//! same way are left to the duplicate alternative check, and those that can
//! both match nothing to the nullable branches check. Rules that take the
//! first match are left alone, since there the first written alternative is
//! meant to win.

use crate::{
    analysis::{
        character_set,
        facts::Facts,
        finding::{Finding, FindingKind},
        grammar_check::unreachable_alternative,
        matcher,
        samples::Sampler,
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar, RuleId},
};

/// Reports every alternative that can match a text an earlier alternative of
/// the same definitions list can match, against the first such alternative.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let mut sampler = Sampler::new(grammar);
    let longest_match =
        grammar.syntax_rules.iter().filter(|rule| !rule.first_match);
    for rule in longest_match {
        rule.expression.visit(&mut |expression| {
            let ExpressionKind::DefinitionsList(alternatives) =
                &expression.kind
            else {
                return;
            };
            for (index, later) in alternatives.iter().enumerate() {
                // An alternative that an earlier one takes every match of is
                // left to the unreachable alternative check.
                if alternatives[..index].iter().any(|earlier| {
                    unreachable_alternative::covers(
                        grammar, facts, rule, earlier, later,
                    )
                }) {
                    continue;
                }
                let tie = alternatives[..index].iter().find_map(|earlier| {
                    shared(&mut sampler, facts, rule.greedy, earlier, later)
                        .map(|matched| (earlier, matched))
                });
                if let Some((earlier, matched)) = tie {
                    let exclusion = written(grammar, later)
                        .zip(written(grammar, earlier))
                        .map(|(later, earlier)| {
                            format!("{later} - {earlier}")
                        });
                    findings.push(Finding {
                        kind: FindingKind::TiedBranches { matched, exclusion },
                        span: later.span,
                        labels: vec![Label {
                            span: earlier.span,
                            message: "this branch can match it too",
                        }],
                    });
                }
            }
        });
    }
}

/// Returns how to write `expression` again, when it is a rule or a terminal
/// string.
fn written(grammar: &Grammar, expression: &Expression) -> Option<String> {
    match &expression.kind {
        ExpressionKind::MetaIdentifier(RuleId(position)) => Some(
            grammar.syntax_rules[*position]
                .meta_identifier
                .written()
                .to_owned(),
        ),
        ExpressionKind::TerminalString(text) if text.contains('\'') => {
            Some(format!("\"{text}\""))
        }
        ExpressionKind::TerminalString(text) => Some(format!("'{text}'")),
        _ => None,
    }
}

/// Returns what both `earlier` and `later`, written in a rule that is `greedy`
/// or not, can match, as a message tells it, when it can be told.
fn shared(
    sampler: &mut Sampler<'_>,
    facts: &Facts<'_>,
    greedy: bool,
    earlier: &Expression,
    later: &Expression,
) -> Option<String> {
    if earlier.is_written_like(later) {
        return None;
    }
    if let (Some(earlier), Some(later)) = (
        facts.single_characters(earlier),
        facts.single_characters(later),
    ) {
        let characters = earlier.intersection(&later);
        return (!characters.is_empty()).then(|| characters.described());
    }
    if facts
        .first(earlier)
        .intersection(&facts.first(later))
        .is_empty()
    {
        return None;
    }
    let grammar = sampler.grammar();
    let matches = |expression, text: &str| {
        matcher::matches_all(grammar, expression, text, greedy) == Some(true)
    };
    // The texts of a branch that only matches a few are all of them, so they
    // tell for sure.
    let listed = [(earlier, later), (later, earlier)].into_iter().find_map(
        |(fixed, other)| Some((matcher::texts(grammar, fixed)?, other)),
    );
    let shared = match listed {
        Some((texts, other)) => texts
            .into_iter()
            .find(|text| !text.is_empty() && matches(other, text)),
        None => sampler.samples(earlier).into_iter().find(|text| {
            !text.is_empty() && matches(earlier, text) && matches(later, text)
        }),
    };
    shared.map(|text| format!("`{}`", character_set::shown(&text)))
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn ties(source: &str) -> Vec<(String, Span, Span)> {
        findings_with(source, FindingCode::TiedBranches)
            .into_iter()
            .map(|finding| {
                (
                    finding.kind.to_string(),
                    finding.span,
                    finding.labels[0].span,
                )
            })
            .collect()
    }

    #[test]
    fn finds_alternatives_that_match_the_same_character() {
        assert_eq!(
            ties("a = ? U+0061..U+007A ? | ('x' | '1');"),
            [(
                "these branches can both match `x`, so the parser takes the \
                 one written first"
                    .to_owned(),
                Span { start: 25, end: 36 },
                Span { start: 4, end: 22 },
            )]
        );
    }

    #[test]
    fn finds_a_keyword_that_an_identifier_matches() {
        let source = "word = keyword | identifier; keyword = 'if' | 'in'; \
                      identifier = letter, {letter}; \
                      letter = ? U+0061..U+007A ?;";
        let found = ties(source);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].0,
            "these branches can both match `if`, so the parser takes the \
             one written first"
        );
    }

    #[test]
    fn tells_how_to_exclude_what_the_branches_share_by_their_names() {
        let source = "word = 'if' | name; name = letter, {letter}; \
                      letter = ? U+0061..U+007A ?;";
        let help = findings_with(source, FindingCode::TiedBranches)[0]
            .kind
            .help();
        assert_eq!(
            help,
            "exclude what they share from one of them with `-`, such as \
             `name - 'if'`"
        );
    }

    #[test]
    fn finds_a_fixed_text_written_after_what_matches_it() {
        let found = ties("a = 'x', {'x'} | ('xx' | 'y');");
        assert_eq!(found.len(), 1);
        assert!(found[0].0.contains("`xx`"), "{found:?}");
    }

    #[test]
    fn finds_a_text_that_two_unbounded_alternatives_match() {
        let source = "token = number | name; number = digit, {digit}; \
                      name = (letter | digit), {letter | digit}; \
                      letter = ? U+0061..U+007A ?; \
                      digit = ? U+0030..U+0039 ?;";
        let found = ties(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].0.contains("`0`"), "{found:?}");
        let apart = "token = number | name; number = digit, {digit}; \
                     name = letter, {letter | digit}; \
                     letter = ? U+0061..U+007A ?; \
                     digit = ? U+0030..U+0039 ?;";
        assert_eq!(ties(apart), []);
    }

    #[test]
    fn accepts_alternatives_that_only_start_alike() {
        assert_eq!(ties("a = 'if' | 'in' | 'i', 'x';"), []);
        assert_eq!(ties("a = 'x', b | 'x', c; b = 'y'; c = 'z';"), []);
    }

    #[test]
    fn accepts_a_keyword_that_an_exception_takes_out() {
        let source = "word = keyword | identifier - keyword; \
                      keyword = 'if'; identifier = letter, {letter}; \
                      letter = ? U+0061..U+007A ?;";
        assert_eq!(ties(source), []);
    }

    #[test]
    fn leaves_duplicates_and_nullable_branches_to_other_checks() {
        assert_eq!(ties("a = 'x' | 'x';"), []);
        assert_eq!(ties("a = ['x'] | ['y'];"), []);
    }

    #[test]
    fn leaves_alternatives_it_cannot_tell_about() {
        assert_eq!(ties("a = {'x'} , 'y' | {'x'}, 'z';"), []);
        assert_eq!(ties("s = a; a = a, 'x' | 'y';"), []);
    }

    #[test]
    fn matches_a_rule_that_takes_the_first_match_as_its_parser_does() {
        let rules = "(* first match *) k = 'i' | 'if'; o = 'if' | 'of';";
        assert_eq!(ties(&format!("s = k | o; {rules}")).len(), 1);
        assert_eq!(ties(&format!("s = k | o; (* greedy *) {rules}")), []);
    }
}
