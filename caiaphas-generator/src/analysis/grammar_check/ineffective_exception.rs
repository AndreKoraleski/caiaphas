//! An ineffective syntactic exception can never apply, such as `'y'` in
//! `a = 'x' - 'y';`, since nothing the factor matches is also matched by the
//! exception.
//!
//! Two expressions can only match the same input if both can match nothing, or
//! both can start with the same character. An exception also never applies
//! when the factor only matches single characters and the exception only
//! longer texts, as `'*/'` in `{char - '*/'}`, or when it matches none of the
//! few texts the factor can.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
        matcher,
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar},
};

/// Reports every syntactic exception that cannot match anything its factor
/// matches.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    grammar.visit(&mut |expression| {
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
        let longer =
            only_longer(grammar, facts, syntactic_factor, syntactic_exception);
        let never =
            matcher::texts(grammar, syntactic_factor).is_some_and(|texts| {
                texts.iter().all(|text| {
                    matcher::excludes(grammar, syntactic_exception, text)
                        == Some(false)
                })
            });
        if shared.is_empty() && !both_empty || longer || never {
            findings.push(Finding {
                kind: FindingKind::IneffectiveException { longer },
                span: syntactic_exception.span,
                labels: vec![Label {
                    span: syntactic_factor.span,
                    message: "nothing this matches starts like the exception",
                }],
            });
        }
    });
}

/// Returns whether `factor` only matches single characters and `exception`
/// only texts of another length.
fn only_longer(
    grammar: &Grammar,
    facts: &Facts<'_>,
    factor: &Expression,
    exception: &Expression,
) -> bool {
    facts.single_characters(factor).is_some()
        && matcher::texts(grammar, exception).is_some_and(|texts| {
            texts.iter().all(|text| text.chars().count() != 1)
        })
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn ineffective(source: &str) -> Vec<(Span, Span)> {
        findings_with(source, FindingCode::IneffectiveException)
            .into_iter()
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
        assert_eq!(ineffective("a = ('xy' | 'xz') - 'xz';"), []);
    }

    #[test]
    fn finds_an_exception_longer_than_the_one_character_of_its_factor() {
        let source = "comment = '/*', {character - '*/'}, '*/'; \
                      character = ? U+0020..U+007E ?;";
        assert_eq!(ineffective(source).len(), 1);
    }

    #[test]
    fn finds_an_exception_that_matches_none_of_the_texts_of_its_factor() {
        assert_eq!(ineffective("a = 'if' - 'i';").len(), 1);
        assert_eq!(ineffective("a = 'xy' - 'xz';").len(), 1);
    }

    #[test]
    fn accepts_an_exception_that_removes_the_empty_match() {
        assert_eq!(ineffective("a = ['x'] - ['y'];"), []);
    }
}
