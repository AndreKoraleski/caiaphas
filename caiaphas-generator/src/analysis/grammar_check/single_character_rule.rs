//! A single character rule only ever matches one character, such as `digit` in
//! `number = digit, {digit};`. When a repetition of one character at a time
//! reads it, every character of the run builds a node and a value of its own,
//! and that can be most of the work of a parser. A repetition of more, such as
//! `{operator, term}`, reads the rule as a part of each round, where its value
//! is often what a type needs.
//!
//! A hidden rule builds neither, and the rules that read it still hold its
//! text, so hiding it makes the parser faster. That makes this a suggestion.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::{ExpressionKind, Grammar, RuleId},
    span::Span,
};

/// Suggests hiding every visible rule that only ever matches one character and
/// that a repetition of one character at a time reads.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        let rule_id = RuleId(position);
        // A converted rule builds a value from each character on purpose.
        if rule.hidden
            || rule.converts_to.is_some()
            || grammar.start_syntax_rules.contains(&rule_id)
            || facts.single_characters(&rule.expression).is_none()
        {
            continue;
        }
        if let Some(repetition) = repetition_reading(grammar, facts, rule_id) {
            findings.push(Finding {
                kind: FindingKind::SingleCharacterRule {
                    meta_identifier: rule.meta_identifier.clone(),
                },
                span: rule.span,
                labels: vec![Label {
                    span: repetition,
                    message: "this repetition reads it",
                }],
            });
        }
    }
}

/// Returns where the first repeated sequence that reads the rule `rule_id` is,
/// if any, of those that only match one character in each round.
fn repetition_reading(
    grammar: &Grammar,
    facts: &Facts<'_>,
    rule_id: RuleId,
) -> Option<Span> {
    let mut found = None;
    grammar.visit(&mut |expression| {
        let ExpressionKind::RepeatedSequence(body) = &expression.kind else {
            return;
        };
        if facts.single_characters(body).is_none() {
            return;
        }
        let mut reads = false;
        body.visit_meta_identifiers(&mut |target, _| {
            reads |= target == rule_id;
        });
        if reads && found.is_none() {
            found = Some(expression.span);
        }
    });
    found
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn suggested(source: &str) -> Vec<(Span, Span)> {
        findings_with(source, FindingCode::SingleCharacterRule)
            .into_iter()
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    #[test]
    fn suggests_hiding_a_rule_of_one_character_a_repetition_reads() {
        assert_eq!(
            suggested("number = digit, {digit}; digit = ? U+0030..U+0039 ?;"),
            [(Span { start: 25, end: 52 }, Span { start: 16, end: 23 })]
        );
        assert_eq!(
            suggested("name = {letter | '_'}; letter = ? U+0061..U+007A ?;")
                .len(),
            1
        );
    }

    #[test]
    fn leaves_a_rule_read_as_one_part_of_each_round() {
        assert_eq!(
            suggested(
                "sum = number, {operator, number}; operator = '+' | '-';\n\
                 (* hide *) number = ? U+0030..U+0039 ?;"
            ),
            []
        );
    }

    #[test]
    fn leaves_rules_that_are_hidden_converted_longer_or_not_repeated_alone() {
        assert_eq!(
            suggested(
                "number = digit, {digit}; (* hide *) digit = 'x' | 'y';"
            ),
            []
        );
        assert_eq!(suggested("pair = digit, {digit}; digit = 'xy';"), []);
        assert_eq!(suggested("pair = digit, digit; digit = 'x';"), []);
        assert_eq!(
            suggested("number = digit, {digit}; (* as u8 *) digit = 'x';"),
            []
        );
    }
}
