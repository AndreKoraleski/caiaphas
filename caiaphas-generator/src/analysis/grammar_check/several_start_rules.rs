//! A grammar has several start rules when rules other than the first are used
//! by no rule. The standard has no start symbol, so that is allowed, but a
//! rule left over, or one misspelled where it is used, looks the same, so a
//! suggestion points them out.

use crate::{
    analysis::{
        facts::Facts,
        finding::{Finding, FindingKind},
    },
    ir::{Grammar, RuleId},
};

/// Reports each start rule after the first, at the rule.
pub(super) fn check(
    grammar: &Grammar,
    _facts: &Facts<'_>,
    findings: &mut Vec<Finding>,
) {
    let mut starts = grammar.start_syntax_rules.iter();
    let Some(&RuleId(first)) = starts.next() else {
        return;
    };
    let first = &grammar.syntax_rules[first].meta_identifier;
    for &RuleId(position) in starts {
        let rule = &grammar.syntax_rules[position];
        findings.push(Finding {
            kind: FindingKind::SeveralStartRules {
                meta_identifier: rule.meta_identifier.clone(),
                first: first.clone(),
            },
            span: rule.span,
            labels: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{finding::FindingCode, tests::findings_with};

    fn several(source: &str) -> usize {
        findings_with(source, FindingCode::SeveralStartRules).len()
    }

    #[test]
    fn suggests_a_look_at_each_start_rule_after_the_first() {
        assert_eq!(several("a = 'x'; b = 'y'; c = 'z';"), 2);
        assert_eq!(several("a = b; b = 'y';"), 0);
        assert_eq!(
            several(
                "a = 'x'; (* expect several-start-rules: a second entry *) \
                 b = 'y';"
            ),
            0
        );
    }
}
