//! A field gathers every match of the rule it holds, wherever its struct holds
//! it. When places of one sequence hold the rule a different number of times,
//! as `name` in `call = name, '(', [name, {',', name}], ')';`, they most
//! likely mean different things, such as a function and its arguments, and the
//! field mixes them. A list such as `item, {',', item}` holds its rule alike
//! in each round, so it counts as one place.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    generate::gatherings,
    ir::{Grammar, RuleId},
};

/// Suggests a label for each place of a sequence that holds a rule whose field
/// would gather matches held a different number of times.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let name = |RuleId(position): RuleId| {
        grammar.syntax_rules[position].meta_identifier.clone()
    };
    for gathering in gatherings(grammar) {
        findings.push(Finding {
            kind: FindingKind::GatheredField {
                meta_identifier: name(gathering.rule),
                held: name(gathering.held),
            },
            span: gathering.sequence,
            labels: gathering
                .places
                .into_iter()
                .map(|span| Label {
                    span,
                    message: "held here",
                })
                .collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingCode, tests::findings_with};

    fn gathered(source: &str) -> usize {
        findings_with(source, FindingCode::GatheredField).len()
    }

    #[test]
    fn suggests_labels_where_a_field_gathers_unlike_places() {
        let name = "name = 'x';";
        assert_eq!(
            gathered(&format!(
                "call = name, '(', [name, {{',', name}}], ')'; {name}"
            )),
            1
        );
        assert_eq!(gathered(&format!("pair = name, [name]; {name}")), 1);
        assert_eq!(
            gathered(&format!(
                "call = callee, '(', [argument, {{',', argument}}], ')'; \
                 (* inline *) callee = name; (* inline *) argument = name; \
                 {name}"
            )),
            0
        );
    }

    #[test]
    fn leaves_lists_and_places_held_alike() {
        assert_eq!(gathered("list = item, {',', item}; item = 'x';"), 0);
        assert_eq!(gathered("sum = term, '+', term; term = 'x';"), 0);
        assert_eq!(
            gathered(
                "pairs = key, '=', value, {',', key, '=', value}; \
                      key = 'k'; value = 'v';"
            ),
            0
        );
        // Places in different alternatives never match together.
        assert_eq!(gathered("s = a, 'x' | [a], 'y'; a = 'a';"), 0);
    }
}
