//! Start rules are the rules a generated parser can start from.

use crate::{
    dictionary::{DictionaryError, DictionaryErrorKind},
    ir::{Rule, RuleId},
};

/// Returns every rule that no other rule references, and reports every start
/// rule that is hidden, converted or inline.
///
/// A rule that only references itself is still a start rule. When every rule
/// is referenced by another, as when the top rule of a grammar is used inside
/// itself, the first rule written is the start rule.
pub(super) fn start_rules(
    rules: &[Rule],
    errors: &mut Vec<DictionaryError>,
) -> Vec<RuleId> {
    let mut referenced = vec![false; rules.len()];
    for (position, rule) in rules.iter().enumerate() {
        rule.expression
            .visit_meta_identifiers(&mut |RuleId(target), _| {
                if target != position {
                    referenced[target] = true;
                }
            });
    }
    let mut start_rules: Vec<RuleId> = (0..rules.len())
        .filter(|&position| !referenced[position])
        .map(RuleId)
        .collect();
    if start_rules.is_empty() && !rules.is_empty() {
        start_rules.push(RuleId(0));
    }
    for &RuleId(position) in &start_rules {
        let rule = &rules[position];
        if rule.converts_to.is_some() {
            errors.push(DictionaryError {
                kind: DictionaryErrorKind::ConvertedStartSyntaxRule {
                    meta_identifier: rule.meta_identifier.clone(),
                },
                span: rule.span,
            });
        }
        if rule.hidden {
            errors.push(DictionaryError {
                kind: DictionaryErrorKind::HiddenStartSyntaxRule {
                    meta_identifier: rule.meta_identifier.clone(),
                },
                span: rule.span,
            });
        }
        if rule.inline {
            errors.push(DictionaryError {
                kind: DictionaryErrorKind::InlineStartSyntaxRule {
                    meta_identifier: rule.meta_identifier.clone(),
                },
                span: rule.span,
            });
        }
    }
    start_rules
}

#[cfg(test)]
mod tests {
    use crate::dictionary::tests::{grammar, start_names};

    fn starts(source: &str) -> Vec<String> {
        let (grammar, errors) = grammar(source);
        assert_eq!(errors, [], "{source:?}");
        start_names(&grammar)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn starts_from_the_only_rule() {
        assert_eq!(starts("a = 'x';"), ["a"]);
    }

    #[test]
    fn starts_from_the_rule_nothing_references() {
        assert_eq!(starts("c = 'x'; b = c; a = b;"), ["a"]);
    }

    #[test]
    fn starts_from_every_rule_nothing_references() {
        assert_eq!(starts("a = c; b = c; c = 'x';"), ["a", "b"]);
    }

    #[test]
    fn starts_from_a_rule_that_only_references_itself() {
        assert_eq!(starts("a = 'x', [a];"), ["a"]);
    }

    #[test]
    fn starts_from_the_first_rule_where_every_rule_is_referenced() {
        assert_eq!(starts("e = t; t = f; f = '(', e, ')' | 'x';"), ["e"]);
    }
}
