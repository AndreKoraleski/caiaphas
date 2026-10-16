//! Syntactic exceptions must be syntactic factors that could be replaced by
//! ones without meta identifiers.

use crate::{
    dictionary::{DictionaryError, DictionaryErrorKind},
    ir::{Expression, ExpressionKind, Rule, RuleId},
};

/// Reports every meta identifier in a syntactic exception that names a rule
/// which could not be replaced by what it matches.
///
/// A rule can be replaced when neither it nor any rule it references is
/// recursive, since then inlining every reference ends.
pub(super) fn check(rules: &[Rule], errors: &mut Vec<DictionaryError>) {
    let mut replaceable = ReplaceableRules {
        rules,
        states: vec![State::Unvisited; rules.len()],
    };
    for rule in rules {
        check_expression(&rule.expression, &mut replaceable, errors);
    }
}

fn check_expression(
    expression: &Expression,
    replaceable: &mut ReplaceableRules<'_>,
    errors: &mut Vec<DictionaryError>,
) {
    if let ExpressionKind::SyntacticTerm {
        syntactic_factor,
        syntactic_exception,
    } = &expression.kind
    {
        // This checks every meta identifier in the exception, even in the
        // exceptions nested inside it, so only the factor is left to check.
        syntactic_exception.visit_meta_identifiers(&mut |rule_id, span| {
            if !replaceable.is_replaceable(rule_id) {
                let RuleId(position) = rule_id;
                errors.push(DictionaryError {
                    kind: DictionaryErrorKind::RecursiveSyntacticException {
                        meta_identifier: replaceable.rules[position]
                            .meta_identifier
                            .clone(),
                    },
                    span,
                });
            }
        });
        check_expression(syntactic_factor, replaceable, errors);
        return;
    }
    for child in expression.children() {
        check_expression(child, replaceable, errors);
    }
}

/// Finds which rules can be replaced, remembering each answer.
struct ReplaceableRules<'a> {
    rules: &'a [Rule],
    states: Vec<State>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Unvisited,
    /// Being checked, so reaching it again means a cycle.
    Visiting,
    Replaceable,
    NotReplaceable,
}

impl ReplaceableRules<'_> {
    fn is_replaceable(&mut self, rule_id: RuleId) -> bool {
        let RuleId(position) = rule_id;
        match self.states[position] {
            State::Replaceable => return true,
            State::NotReplaceable | State::Visiting => return false,
            State::Unvisited => {}
        }
        self.states[position] = State::Visiting;
        let rules = self.rules;
        let mut replaceable = true;
        rules[position]
            .expression
            .visit_meta_identifiers(&mut |target, _| {
                replaceable = replaceable && self.is_replaceable(target);
            });
        self.states[position] = if replaceable {
            State::Replaceable
        } else {
            State::NotReplaceable
        };
        replaceable
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        dictionary::{DictionaryError, DictionaryErrorKind, tests::grammar},
        span::Span,
    };

    fn errors(source: &str) -> Vec<DictionaryError> {
        grammar(source).1
    }

    /// Returns whether `error` says the meta identifier `name` at `start` is
    /// recursive.
    fn is_recursive(
        error: &DictionaryError,
        name: &str,
        start: usize,
    ) -> bool {
        let span = Span {
            start,
            end: start + name.len(),
        };
        matches!(
            &error.kind,
            DictionaryErrorKind::RecursiveSyntacticException {
                meta_identifier,
            } if meta_identifier.as_str() == name
        ) && error.span == span
    }

    #[test]
    fn accepts_exceptions_without_meta_identifiers() {
        assert_eq!(errors("s = 'x' - ('y' | 'z');"), []);
    }

    #[test]
    fn accepts_meta_identifiers_of_rules_that_are_not_recursive() {
        assert_eq!(errors("s = 'x' - b; b = 'y' | c; c = 'z';"), []);
    }

    #[test]
    fn accepts_recursion_outside_the_exception() {
        assert_eq!(errors("s = 'x', [s] - 'y';"), []);
    }

    #[test]
    fn rejects_a_rule_that_references_itself() {
        let errors = errors("s = 'x' - b; b = 'y', [b];");
        assert_eq!(errors.len(), 1);
        assert!(is_recursive(&errors[0], "b", 10), "{errors:?}");
    }

    #[test]
    fn rejects_a_rule_in_a_cycle() {
        let errors = errors("s = 'x' - b; b = c; c = [b];");
        assert_eq!(errors.len(), 1);
        assert!(is_recursive(&errors[0], "b", 10), "{errors:?}");
    }

    #[test]
    fn rejects_a_rule_that_references_a_recursive_rule() {
        let errors = errors("s = 'x' - b; b = c; c = 'y', [c];");
        assert_eq!(errors.len(), 1);
        assert!(is_recursive(&errors[0], "b", 10), "{errors:?}");
    }

    #[test]
    fn rejects_every_recursive_meta_identifier_in_an_exception() {
        let errors = errors("s = 'x' - (b | 'z' | b); b = [b];");
        assert_eq!(errors.len(), 2);
        assert!(is_recursive(&errors[0], "b", 11), "{errors:?}");
        assert!(is_recursive(&errors[1], "b", 21), "{errors:?}");
    }

    #[test]
    fn checks_exceptions_nested_in_other_expressions() {
        let errors = errors("s = ['x' - b]; b = [b];");
        assert_eq!(errors.len(), 1);
        assert!(is_recursive(&errors[0], "b", 11), "{errors:?}");
    }

    #[test]
    fn checks_an_exception_nested_in_an_exception_once() {
        let errors = errors("s = 'x' - ('y' - b); b = [b];");
        assert_eq!(errors.len(), 1);
        assert!(is_recursive(&errors[0], "b", 17), "{errors:?}");
    }

    #[test]
    fn checks_an_exception_nested_in_a_factor() {
        let errors = errors("s = ('x' - b) - 'y'; b = [b];");
        assert_eq!(errors.len(), 1);
        assert!(is_recursive(&errors[0], "b", 11), "{errors:?}");
    }

    #[test]
    fn reports_an_undefined_meta_identifier_only_once() {
        let errors = errors("s = 'x' - b;");
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0].kind,
            DictionaryErrorKind::UndefinedMetaIdentifier { .. }
        ));
    }
}
