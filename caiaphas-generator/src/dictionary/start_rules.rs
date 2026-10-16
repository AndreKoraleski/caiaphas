//! Start rules are the rules a generated parser can start from.

use crate::{
    dictionary::{DictionaryError, DictionaryErrorKind},
    ir::{Rule, RuleId},
    parser::Syntax,
};

/// Returns every rule that no other rule references, and reports a grammar
/// without one and a start rule that is hidden. `rules` are the rules of
/// `syntax`.
///
/// A rule that only references itself is still a start rule.
pub(super) fn start_rules(
    syntax: &Syntax,
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
    let start_rules: Vec<RuleId> = (0..rules.len())
        .filter(|&position| !referenced[position])
        .map(RuleId)
        .collect();
    if start_rules.is_empty() {
        if let (Some(first), Some(last)) =
            (syntax.syntax_rules.first(), syntax.syntax_rules.last())
        {
            errors.push(DictionaryError {
                kind: DictionaryErrorKind::NoStartSyntaxRule,
                span: first.span.to(last.span),
            });
        }
    }
    for &RuleId(position) in &start_rules {
        let rule = &rules[position];
        if rule.hidden {
            errors.push(DictionaryError {
                kind: DictionaryErrorKind::HiddenStartSyntaxRule {
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
    use crate::{
        dictionary::{
            DictionaryErrorKind,
            tests::{grammar, start_names},
        },
        span::Span,
    };

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
    fn rejects_a_grammar_where_every_rule_is_referenced() {
        let source = "e = t; t = f; f = '(', e, ')' | 'x';";
        let (grammar, errors) = grammar(source);
        assert_eq!(grammar.start_syntax_rules, []);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, DictionaryErrorKind::NoStartSyntaxRule);
        assert_eq!(
            errors[0].span,
            Span {
                start: 0,
                end: source.len(),
            }
        );
    }
}
