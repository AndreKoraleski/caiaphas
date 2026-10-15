//! A duplicate alternative is written the same way as an earlier alternative
//! of the same choice, such as the last one in `a = 'x', b | 'y' | 'x', b;`,
//! so both always match the same input.
//!
//! Unlike branches that only start the same way, duplicates are certainly
//! ambiguous. Quotes and grouping do not count, so `'x'` and `("x")` are the
//! same. The order of nested alternatives does, so `(b | c)` and `(c | b)` are
//! not.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar},
};

/// Reports every alternative that an earlier alternative of the same
/// definitions list is written the same way as.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    grammar.visit(&mut |expression| {
        let ExpressionKind::DefinitionsList(alternatives) = &expression.kind
        else {
            return;
        };
        for (index, later) in alternatives.iter().enumerate() {
            if let Some(earlier) = alternatives[..index]
                .iter()
                .find(|earlier| same(earlier, later))
            {
                findings.push(Finding {
                    kind: FindingKind::DuplicateAlternative,
                    span: later.span,
                    labels: vec![Label {
                        span: earlier.span,
                        message: "written the same way as this one",
                    }],
                });
            }
        }
    });
}

/// Returns whether `left` and `right` are written the same way, wherever they
/// are.
fn same(left: &Expression, right: &Expression) -> bool {
    match (&left.kind, &right.kind) {
        (
            ExpressionKind::SingleDefinition(left),
            ExpressionKind::SingleDefinition(right),
        )
        | (
            ExpressionKind::DefinitionsList(left),
            ExpressionKind::DefinitionsList(right),
        ) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same(left, right))
        }
        (
            ExpressionKind::OptionalSequence(left),
            ExpressionKind::OptionalSequence(right),
        )
        | (
            ExpressionKind::RepeatedSequence(left),
            ExpressionKind::RepeatedSequence(right),
        ) => same(left, right),
        (
            ExpressionKind::SyntacticFactor {
                integer: left_integer,
                syntactic_primary: left,
            },
            ExpressionKind::SyntacticFactor {
                integer: right_integer,
                syntactic_primary: right,
            },
        ) => left_integer == right_integer && same(left, right),
        (
            ExpressionKind::SyntacticTerm {
                syntactic_factor: left_factor,
                syntactic_exception: left_exception,
            },
            ExpressionKind::SyntacticTerm {
                syntactic_factor: right_factor,
                syntactic_exception: right_exception,
            },
        ) => {
            same(left_factor, right_factor)
                && same(left_exception, right_exception)
        }
        (
            ExpressionKind::MetaIdentifier(left),
            ExpressionKind::MetaIdentifier(right),
        ) => left == right,
        (
            ExpressionKind::TerminalString(left),
            ExpressionKind::TerminalString(right),
        ) => left == right,
        (
            ExpressionKind::SpecialSequence(left),
            ExpressionKind::SpecialSequence(right),
        ) => left == right,
        (ExpressionKind::EmptySequence, ExpressionKind::EmptySequence) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{finding::FindingCode, tests::findings_with},
        span::Span,
    };

    fn duplicates(source: &str) -> Vec<(Span, Span)> {
        findings_with(source, FindingCode::DuplicateAlternative)
            .into_iter()
            .map(|finding| (finding.span, finding.labels[0].span))
            .collect()
    }

    #[test]
    fn finds_an_alternative_written_twice() {
        assert_eq!(
            duplicates("a = 'x', b | 'y' | 'x', b; b = 'z';"),
            [(Span { start: 19, end: 25 }, Span { start: 4, end: 10 })]
        );
    }

    #[test]
    fn ignores_quotes_and_grouping() {
        assert_eq!(duplicates("a = 'x' | (\"x\");").len(), 1);
        assert_eq!(duplicates("a = ('x', 'y') | 'x', 'y';").len(), 1);
    }

    #[test]
    fn compares_every_kind_of_expression() {
        let source = "a = [b], {c}, 3 * 'd', 'e' - 'f', ? U+0067 ?, \
                      | [b], {c}, 3 * 'd', 'e' - 'f', ? U+0067 ?, ; \
                      b = 'b'; c = 'c';";
        assert_eq!(duplicates(source).len(), 1);
    }

    #[test]
    fn finds_duplicates_in_nested_choices() {
        assert_eq!(duplicates("a = 'x', ('y' | 'z' | 'y');").len(), 1);
    }

    #[test]
    fn reports_each_later_copy_once() {
        assert_eq!(duplicates("a = 'x' | 'x' | 'x';").len(), 2);
    }

    #[test]
    fn accepts_alternatives_that_differ_anywhere() {
        let different = [
            "a = 'x' | 'y';",
            "a = 'x', 'y' | 'x';",
            "a = ['x'] | {'x'};",
            "a = 2 * 'x' | 3 * 'x';",
            "a = 'x' - 'y' | 'x' - 'z';",
            "a = 'w' - 'y' | 'x' - 'y';",
            "a = b | c; b = 'b'; c = 'c';",
            "a = ? U+0061 ? | ? U+0062 ?;",
            "a = ('x' | 'y') | ('y' | 'x');",
            "a = 'x' | ;",
        ];
        for source in different {
            assert_eq!(duplicates(source), [], "{source:?}");
        }
    }
}
