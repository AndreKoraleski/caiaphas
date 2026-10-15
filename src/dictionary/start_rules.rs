//! Start rules are the rules a generated parser can start from.

use crate::{
    dictionary::meta_identifiers,
    ir::{Rule, RuleId},
};

/// Returns every rule that no other rule references.
///
/// A rule that only references itself is still a start rule.
pub(super) fn start_rules(rules: &[Rule]) -> Vec<RuleId> {
    let mut referenced = vec![false; rules.len()];
    for (position, rule) in rules.iter().enumerate() {
        meta_identifiers(&rule.definitions_list, &mut |RuleId(target), _| {
            if target != position {
                referenced[target] = true;
            }
        });
    }
    (0..rules.len())
        .filter(|&position| !referenced[position])
        .map(RuleId)
        .collect()
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
