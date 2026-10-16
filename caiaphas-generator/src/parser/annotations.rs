//! Annotations go to the syntax rule right after them.

use crate::{
    annotation::{Annotation, Comment},
    parser::{ParseError, ParseErrorKind, SyntaxRule},
    span::Span,
};

/// Gives each annotation in the `comments` of `source` to the rule right
/// after it in `rules`, where `attempts` tells where each rule the parser
/// tried to read is, and adds an error for each annotation that is malformed
/// or comes before no rule.
pub(super) fn attach(
    source: &str,
    comments: &[Span],
    attempts: &[Span],
    rules: &mut [SyntaxRule],
    errors: &mut Vec<ParseError>,
) {
    for &span in comments {
        let text = &source[span.range()];
        let annotation = match Comment::read(text, span) {
            Comment::Ordinary => continue,
            Comment::Annotation(annotation) => annotation,
            Comment::MalformedExpectation => {
                errors.push(ParseError {
                    kind: ParseErrorKind::MalformedExpectation,
                    span,
                });
                continue;
            }
            Comment::MalformedHide => {
                errors.push(ParseError {
                    kind: ParseErrorKind::MalformedHide,
                    span,
                });
                continue;
            }
            Comment::MalformedFirstMatch => {
                errors.push(ParseError {
                    kind: ParseErrorKind::MalformedFirstMatch,
                    span,
                });
                continue;
            }
        };
        let inside = attempts
            .iter()
            .any(|rule| rule.start < span.start && span.end < rule.end);
        match attempts.iter().find(|rule| rule.start >= span.end) {
            Some(next) if !inside => {
                let Some(rule) =
                    rules.iter_mut().find(|rule| rule.span == *next)
                else {
                    continue;
                };
                match annotation {
                    Annotation::Expectation(expectation) => {
                        rule.expectations.push(expectation);
                    }
                    Annotation::Hide => rule.hidden = true,
                    Annotation::FirstMatch => rule.first_match = true,
                }
            }
            _ => errors.push(ParseError {
                kind: ParseErrorKind::AnnotationWithoutRule,
                span,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Expected, Syntax, parse, tests::parse_valid};

    fn expectations(syntax: &Syntax) -> Vec<Vec<&str>> {
        syntax
            .syntax_rules
            .iter()
            .map(|rule| {
                rule.expectations
                    .iter()
                    .map(|expectation| expectation.code.as_str())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn attaches_expectations_to_the_rule_after_them() {
        let source = "(* expect empty-rule: kept for later *) a = ; \
                      (* a note *) b = 'x'; \
                      (* expect tied-branches: first *) \
                      (* expect unreachable-rule: second *) c = 'y';";
        let syntax = parse_valid(source);
        assert_eq!(
            expectations(&syntax),
            [
                vec!["empty-rule"],
                vec![],
                vec!["tied-branches", "unreachable-rule"]
            ]
        );
        let expectation = &syntax.syntax_rules[0].expectations[0];
        assert_eq!(expectation.reason, "kept for later");
        assert_eq!(expectation.span, Span { start: 0, end: 39 });
    }

    #[test]
    fn leaves_out_the_expectations_of_a_rule_it_could_not_read() {
        let (syntax, errors) =
            parse("(* expect empty-rule: kept *) a = (b; c = 'y';");
        assert_eq!(expectations(&syntax), [Vec::<&str>::new()]);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::UnexpectedToken(
                    Expected::EndGroupSymbol
                ),
                span: Span { start: 36, end: 37 },
            }]
        );
    }

    #[test]
    fn hides_the_rule_after_a_hide() {
        let syntax = parse_valid(
            "s = a, ws, b; (* hide *) ws = {' '}; a = 'x'; b = 'y';",
        );
        let hidden: Vec<bool> =
            syntax.syntax_rules.iter().map(|rule| rule.hidden).collect();
        assert_eq!(hidden, [false, true, false, false]);
    }

    #[test]
    fn takes_the_first_match_in_the_rule_after_a_first_match() {
        let syntax = parse_valid(
            "s = a, b; (* first match *) a = 'x' | 'xy'; b = 'y';",
        );
        let first_match: Vec<bool> = syntax
            .syntax_rules
            .iter()
            .map(|rule| rule.first_match)
            .collect();
        assert_eq!(first_match, [false, true, false]);
    }

    #[test]
    fn rejects_a_malformed_first_match() {
        let (syntax, errors) = parse("(* first match now *) a = 'x';");
        assert!(!syntax.syntax_rules[0].first_match);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::MalformedFirstMatch,
                span: Span { start: 0, end: 21 },
            }]
        );
    }

    #[test]
    fn rejects_a_malformed_hide() {
        let (syntax, errors) = parse("(* hide spaces *) a = 'x';");
        assert!(!syntax.syntax_rules[0].hidden);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::MalformedHide,
                span: Span { start: 0, end: 17 },
            }]
        );
    }

    #[test]
    fn rejects_a_malformed_expectation() {
        let (syntax, errors) = parse("(* expect empty-rule *) a = 'x';");
        assert_eq!(expectations(&syntax), [Vec::<&str>::new()]);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::MalformedExpectation,
                span: Span { start: 0, end: 23 },
            }]
        );
    }

    #[test]
    fn rejects_an_annotation_without_a_rule_after_it() {
        for (source, start) in [
            ("a = 'x'; (* expect empty-rule: kept *)", 9),
            ("a = 'x', (* expect empty-rule: kept *) 'y';", 9),
            ("a = 'x', (* expect empty-rule: kept *) 'y'; b = 'z';", 9),
        ] {
            let (syntax, errors) = parse(source);
            assert!(
                syntax
                    .syntax_rules
                    .iter()
                    .all(|rule| rule.expectations.is_empty())
            );
            assert_eq!(
                errors,
                [ParseError {
                    kind: ParseErrorKind::AnnotationWithoutRule,
                    span: Span {
                        start,
                        end: start + 29,
                    },
                }],
                "{source:?}"
            );
        }
        let (syntax, errors) = parse("a = 'x', (* hide *) 'y'; b = 'z';");
        assert!(syntax.syntax_rules.iter().all(|rule| !rule.hidden));
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::AnnotationWithoutRule,
                span: Span { start: 9, end: 19 },
            }]
        );
    }
}
