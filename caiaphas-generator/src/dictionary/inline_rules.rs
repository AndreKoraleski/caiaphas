//! Inline rules give the rules that hold them what they hold, with no type of
//! their own. A parser cannot start from one, since its `parse` returns its
//! type, and one cannot hold itself through inline rules alone, or what it
//! gives would never end.

use crate::{
    dictionary::{DictionaryError, DictionaryErrorKind},
    ir::{Rule, RuleId, reachable},
};

/// Reports every start rule that is inline, and every inline rule that holds
/// itself through inline rules alone.
pub(super) fn check(
    rules: &[Rule],
    start_rules: &[RuleId],
    errors: &mut Vec<DictionaryError>,
) {
    for &RuleId(position) in start_rules {
        let rule = &rules[position];
        if rule.inline {
            errors.push(DictionaryError {
                kind: DictionaryErrorKind::InlineStartSyntaxRule {
                    meta_identifier: rule.meta_identifier.clone(),
                },
                span: rule.span,
            });
        }
    }
    let held = |RuleId(position): RuleId, pending: &mut Vec<RuleId>| {
        rules[position].expression.visit_meta_identifiers(
            &mut |RuleId(read), _| {
                if rules[read].inline && !rules[read].hidden {
                    pending.push(RuleId(read));
                }
            },
        );
    };
    for (position, rule) in rules.iter().enumerate() {
        if !rule.inline || rule.hidden {
            continue;
        }
        let mut starts = Vec::new();
        held(RuleId(position), &mut starts);
        if reachable(rules.len(), starts, held)[position] {
            errors.push(DictionaryError {
                kind: DictionaryErrorKind::RecursiveInlineSyntaxRule {
                    meta_identifier: rule.meta_identifier.clone(),
                },
                span: rule.span,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dictionary::{DictionaryErrorKind, tests::grammar};

    fn errors(source: &str) -> Vec<String> {
        grammar(source)
            .1
            .into_iter()
            .map(|error| match error.kind {
                DictionaryErrorKind::InlineStartSyntaxRule {
                    meta_identifier,
                } => format!("start {}", meta_identifier.as_str()),
                DictionaryErrorKind::RecursiveInlineSyntaxRule {
                    meta_identifier,
                } => format!("recursive {}", meta_identifier.as_str()),
                kind => format!("{kind:?}"),
            })
            .collect()
    }

    #[test]
    fn reports_an_inline_start_rule() {
        assert_eq!(errors("(* inline *) s = a; a = 'x';"), ["start s"]);
    }

    #[test]
    fn reports_an_inline_rule_that_holds_itself_through_inline_rules() {
        assert_eq!(
            errors(
                "s = a; (* inline *) a = 'x', [b]; (* inline *) b = 'y', a;"
            ),
            ["recursive a", "recursive b"]
        );
        assert_eq!(
            errors("s = a; (* inline *) a = 'x', [b]; b = 'y', a;"),
            Vec::<String>::new()
        );
    }
}
