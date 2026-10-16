//! Tied branches are alternatives that can match the same text, such as `'if'`
//! and `identifier` in `word = 'if' | identifier;`. A parser takes the
//! alternative that matches the most, so it only has to pick between two when
//! they tie, and then it takes the one written first.
//!
//! Whether two alternatives can match the same text is only told when one of
//! them matches single characters or a few fixed texts, so other alternatives
//! are left alone. Alternatives written the same way are left to the duplicate
//! alternative check, and those that can both match nothing to the nullable
//! branches check. Rules that take the first match are left alone, since there
//! the first written alternative is meant to win.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
        matcher,
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar},
};

/// Reports every alternative that can match a text an earlier alternative of
/// the same definitions list can match, against the first such alternative.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
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
                let tie = alternatives[..index].iter().find_map(|earlier| {
                    shared(grammar, facts, earlier, later)
                        .map(|matched| (earlier, matched))
                });
                if let Some((earlier, matched)) = tie {
                    findings.push(Finding {
                        kind: FindingKind::TiedBranches { matched },
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

/// Returns what both `earlier` and `later` can match, written as it would
/// appear in Rust, when it can be told.
fn shared(
    grammar: &Grammar,
    facts: &Facts<'_>,
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
        return (!characters.is_empty()).then(|| characters.to_string());
    }
    [(earlier, later), (later, earlier)]
        .into_iter()
        .find_map(|(fixed, other)| {
            matcher::texts(grammar, fixed)?.into_iter().find(|text| {
                !text.is_empty()
                    && matcher::matches_all(grammar, other, text) == Some(true)
            })
        })
        .map(|text| format!("{text:?}"))
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
            ties("a = ? U+0061..U+007A ? | 'x';"),
            [(
                "these branches can both match 'x', so the parser takes the \
                 one written first"
                    .to_owned(),
                Span { start: 25, end: 28 },
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
            "these branches can both match \"if\", so the parser takes the \
             one written first"
        );
    }

    #[test]
    fn finds_a_fixed_text_written_after_what_matches_it() {
        let found = ties("a = 'x', {'x'} | 'xx';");
        assert_eq!(found.len(), 1);
        assert!(found[0].0.contains("\"xx\""), "{found:?}");
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
        assert_eq!(
            ties("s = k | o; (* first match *) k = 'i' | 'if'; o = 'if';"),
            []
        );
    }
}
