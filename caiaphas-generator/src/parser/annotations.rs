//! Annotations go to the syntax rule right after them.

use crate::{
    annotation::{Annotation, Comment, Malformed, Misspelling},
    parser::{ParseError, ParseErrorKind, SyntaxRule},
    span::Span,
};

/// Gives each annotation in the `comments` of `source` to the rule right after
/// it in `rules`, where `attempts` tells where each rule the parser tried to
/// read is, and adds an error for each annotation that is malformed or comes
/// before no rule. A comment inside a rule is text for people alone, as every
/// comment is to the standard.
pub(super) fn attach(
    source: &str,
    comments: &[Span],
    attempts: &[Span],
    rules: &mut [SyntaxRule],
    errors: &mut Vec<ParseError>,
) {
    for &span in comments {
        let inside = attempts
            .iter()
            .any(|rule| rule.start < span.start && span.end < rule.end);
        if inside {
            continue;
        }
        let kind = match Comment::read(&source[span.range()], span) {
            Comment::Ordinary => {
                document(source, span, attempts, rules);
                continue;
            }
            Comment::Annotation(annotation) => {
                if give(annotation, span, attempts, rules) {
                    continue;
                }
                ParseErrorKind::AnnotationWithoutRule
            }
            // A word written almost as an annotation is likely a typo, which a
            // warning about the rule it touches tells of.
            Comment::Malformed(Malformed::Misspelled { meant }) => {
                if let Some(rule) = touched(source, span, attempts, rules) {
                    rule.misspellings.push(Misspelling { meant, span });
                }
                continue;
            }
            Comment::Malformed(malformed) => {
                ParseErrorKind::Malformed(malformed)
            }
        };
        errors.push(ParseError { kind, span });
    }
}

/// Gives the text of the ordinary comment at `span` of `source` to the rule it
/// touches, so a comment set apart, such as a header, stays a comment.
fn document(
    source: &str,
    span: Span,
    attempts: &[Span],
    rules: &mut [SyntaxRule],
) {
    let Some(rule) = touched(source, span, attempts, rules) else {
        return;
    };
    let text = &source[span.start + 2..span.end - 2];
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let text = lines.join("\n").trim().to_owned();
    if !text.is_empty() {
        rule.comments.push(text);
    }
}

/// Returns the rule that the comment at `span` of `source` touches, with
/// nothing but gaps and comments between them and no empty line.
fn touched<'r>(
    source: &str,
    span: Span,
    attempts: &[Span],
    rules: &'r mut [SyntaxRule],
) -> Option<&'r mut SyntaxRule> {
    let next = attempts.iter().find(|rule| rule.start >= span.end)?;
    let between = &source[span.end..next.start];
    // The lines wholly between the two are those past the comment's own line
    // and before the rule's, and none of them can be empty.
    let lines: Vec<&str> = between.split('\n').collect();
    let inner = lines
        .get(1..lines.len().saturating_sub(1))
        .unwrap_or_default();
    if inner.iter().any(|line| line.trim().is_empty()) {
        return None;
    }
    rules.iter_mut().find(|rule| rule.span == *next)
}

/// Gives `annotation`, written at `span`, to the rule right after it, and
/// returns whether there is a rule right after it. A rule the parser could not
/// read is left out with its annotations.
fn give(
    annotation: Annotation,
    span: Span,
    attempts: &[Span],
    rules: &mut [SyntaxRule],
) -> bool {
    let Some(next) = attempts.iter().find(|rule| rule.start >= span.end)
    else {
        return false;
    };
    if let Some(rule) = rules.iter_mut().find(|rule| rule.span == *next) {
        match annotation {
            Annotation::Expectation(expectation) => {
                rule.expectations.push(expectation);
            }
            Annotation::Hide => rule.hidden = true,
            Annotation::Greedy => rule.greedy = true,
            Annotation::Recover => rule.recovers = true,
            Annotation::Inline => rule.inline = true,
            Annotation::Converts(ty) => rule.converts_to = Some(ty),
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        annotation::Malformed,
        parser::{Expected, Syntax, parse, tests::parse_valid},
    };

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
    fn reads_prose_after_an_annotation_word_as_a_comment() {
        let (syntax, errors) = parse("(* hide spaces *) a = 'x';");
        assert!(!syntax.syntax_rules[0].hidden);
        assert_eq!(errors, []);
        assert_eq!(syntax.syntax_rules[0].comments, ["hide spaces"]);
    }

    #[test]
    fn leaves_a_comment_inside_a_rule_alone() {
        let (syntax, errors) =
            parse("a = 'x' (* expect a digit next *), (* hide *) 'y';");
        assert_eq!(errors, []);
        assert!(!syntax.syntax_rules[0].hidden);
    }

    #[test]
    fn notes_a_word_written_almost_as_an_annotation_for_its_rule() {
        let (syntax, errors) =
            parse("(* HIDE *) a = 'x';\n\n(* Hidde *)\n\nb = 'y';");
        assert_eq!(errors, []);
        assert_eq!(
            syntax.syntax_rules[0].misspellings,
            [Misspelling {
                meant: "hide",
                span: Span { start: 0, end: 10 },
            }]
        );
        // One set apart by an empty line is a header.
        assert_eq!(syntax.syntax_rules[1].misspellings, []);
    }

    #[test]
    fn rejects_a_malformed_expectation() {
        let (syntax, errors) = parse("(* expect empty-rule *) a = 'x';");
        assert_eq!(expectations(&syntax), [Vec::<&str>::new()]);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::Malformed(Malformed::NoColon),
                span: Span { start: 0, end: 23 },
            }]
        );
    }

    #[test]
    fn rejects_an_annotation_without_a_rule_after_it() {
        let (syntax, errors) = parse("a = 'x'; (* expect empty-rule: kept *)");
        assert_eq!(syntax.syntax_rules[0].expectations, []);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::AnnotationWithoutRule,
                span: Span { start: 9, end: 38 },
            }]
        );
        // One inside a rule is a comment like any other.
        for source in [
            "a = 'x', (* expect empty-rule: kept *) 'y'; b = 'z';",
            "a = 'x', (* hide *) 'y'; b = 'z';",
        ] {
            let (syntax, errors) = parse(source);
            assert_eq!(errors, [], "{source:?}");
            assert!(syntax.syntax_rules.iter().all(|rule| !rule.hidden));
            assert!(
                syntax
                    .syntax_rules
                    .iter()
                    .all(|rule| rule.expectations.is_empty())
            );
        }
    }

    #[test]
    fn gives_a_rule_the_comments_that_touch_it() {
        let syntax = parse_valid(
            "(* A header. *)\n\n(* A greeting,\n   to say. *)\n\
             (* hide *)\ngreeting = 'hi';",
        );
        assert_eq!(syntax.syntax_rules[0].comments, ["A greeting,\nto say."]);
        assert!(syntax.syntax_rules[0].hidden);
    }
}
