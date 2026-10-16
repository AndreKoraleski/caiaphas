//! A nondeterministic choice is one that a parser cannot make from what comes
//! next, such as whether to repeat `{'x'}` in `{'x'}, 'x'`. Its rule tries
//! every way through it, and that is slower than taking one way as it comes.

use std::collections::HashMap;

use super::longer_branch_conflict;
use crate::{
    analysis::{
        choice_check::CHOICE_CHECKS,
        choice_point::choice_points,
        exactness::visit_conflicts,
        facts::Facts,
        finding::{Finding, FindingCode, FindingKind},
    },
    diagnostic::Label,
    ir::{Grammar, RuleId},
};

/// Suggests a way to keep the parser fast at each choice that makes a rule try
/// every way through it.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let mut greedy = HashMap::new();
    visit_conflicts(facts, &mut |conflict| {
        let RuleId(position) = conflict.rule_id;
        let meta_identifier =
            grammar.syntax_rules[position].meta_identifier.clone();
        let greedy = greedy
            .entry(position)
            .or_insert_with(|| greedy_findings(grammar, conflict.rule_id))
            .clone();
        let labels = conflict.shorter.map(|span| Label {
            span,
            message: "this alternative ends where that one goes on",
        });
        findings.push(Finding {
            kind: FindingKind::NondeterministicChoice {
                meta_identifier,
                characters: conflict.characters,
                greedy,
            },
            span: conflict.span,
            labels: labels.into_iter().collect(),
        });
    });
}

/// Returns the codes of the findings that `(* greedy *)` right before the rule
/// `rule_id` would bring about in it, or `None` when one of them means the
/// rule would then miss what it matches.
fn greedy_findings(
    grammar: &Grammar,
    rule_id: RuleId,
) -> Option<Vec<FindingCode>> {
    let RuleId(position) = rule_id;
    let mut greedy = grammar.clone();
    greedy.syntax_rules[position].greedy = true;
    let facts = Facts::new(&greedy);
    let mut findings = Vec::new();
    longer_branch_conflict::check(&greedy, &facts, &mut findings);
    for choice_point in choice_points(&facts) {
        for check in CHOICE_CHECKS {
            check(&choice_point, &mut findings);
        }
    }
    let span = grammar.syntax_rules[position].span;
    let mut codes: Vec<FindingCode> = findings
        .iter()
        .filter(|finding| {
            span.start <= finding.span.start && finding.span.end <= span.end
        })
        .map(|finding| finding.kind.code())
        .filter(|code| code.is_greedy_only())
        .collect();
    codes.sort_unstable();
    codes.dedup();
    let misses = codes.iter().any(|code| {
        matches!(
            code,
            FindingCode::RequiredOptionalSequence
                | FindingCode::DeadEndRepeatedSequence
        )
    });
    (!misses).then_some(codes)
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn choices(source: &str) -> Vec<(String, Span, Vec<Span>)> {
        findings_with(source, FindingCode::NondeterministicChoice)
            .into_iter()
            .map(|finding| {
                let labels = finding.labels.iter().map(|label| label.span);
                (finding.kind.to_string(), finding.span, labels.collect())
            })
            .collect()
    }

    #[test]
    fn suggests_greedy_where_a_sequence_starts_like_what_follows_it() {
        assert_eq!(
            choices("s = {'x'}, 'x';"),
            [(
                "a parser cannot tell from `x` which way to take here, so \
                 `s` and the rules that reach it try every way"
                    .to_owned(),
                Span { start: 4, end: 9 },
                Vec::new()
            )]
        );
    }

    #[test]
    fn points_at_the_alternative_that_goes_on_where_another_ends() {
        let found = choices("s = ('x' | 'x', 'y'), 'y';");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, Span { start: 11, end: 19 });
        assert_eq!(found[0].2, [Span { start: 5, end: 8 }]);
    }

    #[test]
    fn suggests_greedy_with_the_expectation_it_then_needs() {
        let source = "s = statement; c = 'c'; statement = 'if', c, \
                      statement, ['else', statement] | 'x';";
        let help = findings_with(source, FindingCode::NondeterministicChoice)
            [0]
        .kind
        .help();
        assert_eq!(
            help,
            "write `(* greedy *)` and `(* expect optional-follow-conflict: \
             reason *)` right before `statement` if it should take each way \
             as it comes, which keeps its parser fast, or make what comes \
             after the choice start differently"
        );
    }

    #[test]
    fn leaves_out_a_greedy_rule_and_a_choice_the_next_character_decides() {
        assert_eq!(choices("(* greedy *) s = {'x'}, 'x';"), []);
        assert_eq!(choices("s = {'x'}, 'y';"), []);
    }
}
