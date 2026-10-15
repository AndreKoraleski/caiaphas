//! An ineffective syntactic exception can never apply, such as `'y'` in
//! `a = 'x' - 'y';`, since nothing the factor matches is also matched by the
//! exception.
//!
//! Two expressions can only match the same input if both can match nothing, or
//! both can start with the same character. When neither holds, the exception
//! removes nothing and is dead code in the grammar.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind, Label},
    },
    ir::{ExpressionKind, Grammar},
};

/// Reports every syntactic exception that cannot match anything its factor
/// matches.
pub(super) fn check(
    _grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    facts.walk(&mut |expression, _| {
        let ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } = &expression.kind
        else {
            return;
        };
        let shared = facts
            .first(syntactic_factor)
            .intersection(&facts.first(syntactic_exception));
        let both_empty = facts.is_nullable(syntactic_factor)
            && facts.is_nullable(syntactic_exception);
        if shared.is_empty() && !both_empty {
            findings.push(Finding {
                kind: FindingKind::IneffectiveException,
                span: syntactic_exception.span,
                labels: vec![Label {
                    span: syntactic_factor.span,
                    message: "nothing this matches starts like the exception",
                }],
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingKind, tests::findings},
        span::Span,
    };

    fn ineffective(source: &str) -> Vec<(Span, Span)> {
        findings(source)
            .into_iter()
            .filter(|finding| {
                finding.kind == FindingKind::IneffectiveException
            })
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    #[test]
    fn finds_an_exception_that_starts_differently() {
        assert_eq!(
            ineffective("a = 'x' - 'y';"),
            [(Span { start: 10, end: 13 }, Span { start: 4, end: 7 })]
        );
    }

    #[test]
    fn finds_an_exception_that_can_only_match_nothing_extra() {
        assert_eq!(ineffective("a = 'x' - ['y'];").len(), 1);
    }

    #[test]
    fn accepts_an_exception_that_starts_like_the_factor() {
        assert_eq!(ineffective("a = ? U+0061..U+007A ? - 'x';"), []);
        assert_eq!(ineffective("a = 'xy' - 'xz';"), []);
    }

    #[test]
    fn accepts_an_exception_that_removes_the_empty_match() {
        assert_eq!(ineffective("a = ['x'] - ['y'];"), []);
    }
}
