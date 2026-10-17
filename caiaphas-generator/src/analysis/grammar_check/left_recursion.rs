//! Left recursion makes a parser call a rule from inside itself before reading
//! anything, so it never ends. The help rewrites a rule whose alternatives
//! start with the rule itself as a repetition, and keeps the terminal strings
//! that start each repeated part apart in a rule of their own, as the
//! operators of
//! `expression = expression, '+', term | expression, '-', term | term;`, so no
//! choice of them leaves its type without a trace.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    diagnostic::Label,
    ir::{Expression, ExpressionKind, Grammar, Rule, RuleId},
};

/// Reports every rule that can reach itself before reading a character,
/// directly or behind parts that can match nothing, once, where it first does.
pub(super) fn check(
    grammar: &Grammar,
    facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    for (position, rule) in grammar.syntax_rules.iter().enumerate() {
        let mut spans = facts
            .first_meta_identifiers(&rule.expression)
            .into_iter()
            .filter(|&(target, _)| target == RuleId(position))
            .map(|(_, span)| span);
        let Some(span) = spans.next() else {
            continue;
        };
        let mut labels = vec![Label {
            span: rule.span,
            message: "in this rule",
        }];
        labels.extend(spans.map(|span| Label {
            span,
            message: "and here",
        }));
        findings.push(Finding {
            kind: FindingKind::LeftRecursion {
                meta_identifier: rule.meta_identifier.clone(),
                rewrite: rewrite(grammar, RuleId(position), rule),
            },
            span,
            labels,
        });
    }
}

/// Returns `rule` written as a repetition, when its alternatives that start
/// with the rule itself do so without parts that can match nothing before it,
/// and some alternatives do not. When each of those goes on with a terminal
/// string and then the same parts, a rule of their own holds the terminal
/// strings.
fn rewrite(grammar: &Grammar, rule_id: RuleId, rule: &Rule) -> Option<String> {
    let alternatives = match &rule.expression.kind {
        ExpressionKind::DefinitionsList(alternatives) => {
            alternatives.as_slice()
        }
        _ => std::slice::from_ref(&rule.expression),
    };
    let starts_with_itself = |alternative: &Expression| {
        matches!(
            alternative.steps().first().map(|step| &step.kind),
            Some(ExpressionKind::MetaIdentifier(target)) if *target == rule_id
        )
    };
    let (recursive, bases): (Vec<&Expression>, Vec<&Expression>) =
        alternatives
            .iter()
            .partition(|alternative| starts_with_itself(alternative));
    if bases.is_empty() || recursive.is_empty() {
        return None;
    }
    let written = |expressions: &[&Expression]| -> Option<String> {
        let parts: Option<Vec<String>> = expressions
            .iter()
            .map(|expression| expression.written(grammar))
            .collect();
        Some(parts?.join(", "))
    };
    let base = if let [base] = bases[..] {
        base.written(grammar)?
    } else {
        let parts: Option<Vec<String>> =
            bases.iter().map(|base| base.written(grammar)).collect();
        format!("({})", parts?.join(" | "))
    };
    let rests: Vec<&[Expression]> = recursive
        .iter()
        .map(|alternative| &alternative.steps()[1..])
        .collect();
    let name = rule.meta_identifier.spaced();
    // The terminal strings that start the repeated parts, when the rest of
    // each part is written alike.
    let operators: Option<Vec<&Expression>> = rests
        .iter()
        .map(|rest| match rest {
            [first, ..]
                if matches!(first.kind, ExpressionKind::TerminalString(_)) =>
            {
                Some(first)
            }
            _ => None,
        })
        .collect();
    let tail: Vec<&Expression> = rests[0].iter().skip(1).collect();
    let alike = rests.iter().all(|rest| {
        rest.len() == tail.len() + 1
            && rest[1..]
                .iter()
                .zip(&tail)
                .all(|(part, other)| part.is_written_like(other))
    });
    match operators {
        Some(operators) if rests.len() > 1 && alike => {
            let operators: Option<Vec<String>> = operators
                .iter()
                .map(|operator| operator.written(grammar))
                .collect();
            let tail = if tail.is_empty() {
                String::new()
            } else {
                format!(", {}", written(&tail)?)
            };
            Some(format!(
                "{name} = {base}, {{{name} operator{tail}}}; {name} operator \
                 = {};",
                operators?.join(" | ")
            ))
        }
        _ => {
            let parts: Option<Vec<String>> = rests
                .iter()
                .map(|rest| written(&rest.iter().collect::<Vec<_>>()))
                .collect();
            Some(format!("{name} = {base}, {{{}}};", parts?.join(" | ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        analysis::{
            finding::{FindingCode, FindingKind},
            tests::{findings, findings_with},
        },
        diagnostic::Label,
        span::Span,
    };

    fn left_recursions(source: &str) -> Vec<Span> {
        findings_with(source, FindingCode::LeftRecursion)
            .into_iter()
            .map(|finding| finding.span)
            .collect()
    }

    #[test]
    fn finds_a_rule_that_starts_with_itself() {
        assert_eq!(
            left_recursions("a = a, 'x' | 'y';"),
            [Span { start: 4, end: 5 }]
        );
    }

    #[test]
    fn finds_a_rule_that_starts_with_itself_behind_nullable_parts() {
        assert_eq!(
            left_recursions("a = ['x'], {'y'}, a, 'z' | 'w';"),
            [Span { start: 18, end: 19 }]
        );
    }

    #[test]
    fn points_at_the_rule() {
        let finding = findings("b = 'x'; a = a, 'x' | 'y';")
            .into_iter()
            .find(|finding| {
                matches!(finding.kind, FindingKind::LeftRecursion { .. })
            })
            .expect("a is left recursive");
        assert_eq!(
            finding.labels,
            [Label {
                span: Span { start: 9, end: 26 },
                message: "in this rule",
            }]
        );
    }

    fn help(source: &str) -> String {
        findings_with(source, FindingCode::LeftRecursion)
            .remove(0)
            .kind
            .help()
    }

    #[test]
    fn rewrites_the_rule_and_keeps_its_operators_apart() {
        assert_eq!(
            help(
                "expression = expression, '+', term | expression, '-', term \
                 | term; term = 'x';"
            ),
            "rewrite the recursion as a repetition: `expression = term, \
             {expression operator, term}; expression operator = '+' | '-';`"
        );
        assert_eq!(
            help("list = list, ',', item | item; item = 'x';"),
            "rewrite the recursion as a repetition: `list = item, {',', \
             item};`"
        );
        assert_eq!(
            help("a = a, ? /[a-z]/ ? | 'y';"),
            "rewrite the recursion as a repetition, so `a = a, x | y;` \
             becomes `a = y, {x};`"
        );
    }

    #[test]
    fn reports_a_rule_once() {
        assert_eq!(left_recursions("a = a, 'x' | a, 'y' | 'z';").len(), 1);
    }

    #[test]
    fn accepts_a_rule_that_reads_before_itself() {
        assert_eq!(left_recursions("a = 'x', a | 'y';"), []);
        assert_eq!(left_recursions("a = 'x', [a];"), []);
    }
}
