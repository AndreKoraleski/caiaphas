//! The analysis finds what in a grammar would make its parser fail, choose
//! ambiguously, or not match what the grammar seems to mean.
//!
//! It runs the grammar checks, then the choice checks on every choice point,
//! then leaves out the findings that expectations mark as intended.

mod character_set;
mod choice_check;
mod choice_point;
#[cfg(test)]
mod corpus;
mod expectations;
mod facts;
mod finding;
mod grammar_check;
mod matcher;

use choice_check::CHOICE_CHECKS;
pub(crate) use facts::Facts;
pub(crate) use finding::Finding;
use grammar_check::GRAMMAR_CHECKS;

use crate::ir::Grammar;

/// Returns every finding about `grammar`, in the order they appear in its
/// source, and those at the same place in the order of their codes.
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
    expectations::apply(grammar, &mut findings);
    findings.sort_by_key(|finding| (finding.span.start, finding.kind.code()));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::finding::FindingCode, dictionary::tests::build_valid,
    };

    /// Returns the findings about the grammar `source` defines, asserting it
    /// has no errors before the analysis.
    pub(super) fn findings(source: &str) -> Vec<Finding> {
        analyse(&build_valid(source))
    }

    /// Returns the findings with `code` about the grammar `source` defines.
    pub(super) fn findings_with(
        source: &str,
        code: FindingCode,
    ) -> Vec<Finding> {
        findings(source)
            .into_iter()
            .filter(|finding| finding.kind.code() == code)
            .collect()
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
