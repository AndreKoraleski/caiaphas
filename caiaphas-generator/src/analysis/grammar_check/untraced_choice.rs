//! A choice leaves no trace in a type when its alternatives hold the same
//! rules as often, as `'+'` and `'-'` do in
//! `sum = term, {('+' | '-'), term};`. The fields of `sum` then hold every
//! `term` and nothing else, so only its text tells which operator came between
//! them. A rule of its own for what tells the alternatives apart, as
//! `operator = '+' | '-';`, becomes an enum that does. Only a choice between
//! places that hold rules is warned of, as an operator between its operands
//! is, since one elsewhere, such as `('e' | 'E')` at the start of an exponent,
//! mostly writes one thing in several ways.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    generate::untraced_choices,
    ir::{Grammar, RuleId},
};

/// Warns of every choice between places that hold rules whose alternatives
/// hold the same rules as often.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for untraced in untraced_choices(grammar) {
        let RuleId(position) = untraced.rule;
        findings.push(Finding {
            kind: FindingKind::UntracedChoice {
                meta_identifier: grammar.syntax_rules[position]
                    .meta_identifier
                    .clone(),
            },
            span: untraced.choice,
            labels: untraced
                .alternatives
                .into_iter()
                .map(|span| Label {
                    span,
                    message: "holds what another alternative holds",
                })
                .collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingCode, tests::findings_with};

    fn untraced(source: &str) -> usize {
        findings_with(source, FindingCode::UntracedChoice).len()
    }

    #[test]
    fn warns_of_a_choice_whose_alternatives_hold_alike() {
        let term = "term = ? /[0-9]+/ ?;";
        assert_eq!(
            untraced(&format!("sum = term, {{('+' | '-'), term}}; {term}")),
            1
        );
        assert_eq!(
            untraced(&format!(
                "sum = term, {{'+', term | '-', term}}; {term}"
            )),
            1
        );
        assert_eq!(
            untraced(&format!(
                "sum = term, {{operator, term}}; operator = '+' | '-'; {term}"
            )),
            0
        );
    }

    #[test]
    fn leaves_a_choice_its_fields_or_text_tell() {
        // Holding a rule or not tells the alternatives apart.
        assert_eq!(untraced("s = a, ('x' | b); a = 'a'; b = 'b';"), 0);
        // A struct without fields is read by its text.
        assert_eq!(untraced("sign = '+' | '-', 'x';"), 0);
        assert_eq!(untraced("s = a, ('x' | 'x'), a; a = 'a';"), 0);
        // A choice that no part holding rules comes before writes one thing in
        // several ways.
        assert_eq!(untraced("e = ('e' | 'E'), digit; digit = 'd';"), 0);
        assert_eq!(
            untraced("log = {line, ('x' | ? end ?)}; line = ? /[a-w]+/ ?;"),
            0
        );
    }
}
