//! The analysis finds what in a grammar would make its parser fail or choose
//! ambiguously.
//!
//! It runs two kinds of checks. Grammar checks read the whole grammar, such as
//! those about recursion. Choice checks read one choice point at a time, where
//! a parser has to pick a branch, and compare what each branch can start with.
//! A new check goes in its own file under `grammar_check` or `choice_check`,
//! and in the list there.

mod character_set;
mod choice_check;
mod choice_point;
#[cfg(test)]
mod corpus;
mod facts;
mod finding;
mod grammar_check;

use choice_check::CHOICE_CHECKS;
use facts::Facts;
pub(crate) use finding::Finding;
use grammar_check::GRAMMAR_CHECKS;

use crate::ir::Grammar;

/// Returns every finding about `grammar`, in the order they appear in its
/// source.
///
/// The grammar should be one the dictionary built without errors.
pub(crate) fn analyse(grammar: &Grammar) -> Vec<Finding> {
    let facts = Facts::new(grammar);
    let mut findings = Vec::new();
    for check in GRAMMAR_CHECKS {
        check(grammar, &facts, &mut findings);
    }
    for choice_point in choice_point::choice_points(&facts) {
        for check in CHOICE_CHECKS {
            check(&choice_point, &mut findings);
        }
    }
    findings.sort_by_key(|finding| finding.span.start);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dictionary::build, parser::parse};

    /// Returns the findings about the grammar `source` defines, asserting it
    /// has no errors before the analysis.
    pub(super) fn findings(source: &str) -> Vec<Finding> {
        let (syntax, errors) = parse(source);
        assert_eq!(errors, [], "{source:?}");
        let (grammar, errors) = build(&syntax);
        assert_eq!(errors, [], "{source:?}");
        analyse(&grammar)
    }

    #[test]
    fn finds_nothing_in_a_grammar_without_problems() {
        assert_eq!(findings("a = 'x', b; b = 'y' | 'z';"), []);
    }

    #[test]
    fn sorts_findings_by_where_they_are() {
        let starts: Vec<usize> =
            findings("a = ['x'] | ['y']; b = a, b | 'z';")
                .iter()
                .map(|finding| finding.span.start)
                .collect();
        let mut sorted = starts.clone();
        sorted.sort_unstable();
        assert_eq!(starts, sorted);
        assert!(!starts.is_empty());
    }
}
