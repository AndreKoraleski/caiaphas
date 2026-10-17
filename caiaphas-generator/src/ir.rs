//! The intermediate representation of a grammar, as analysis and code
//! generation read it.

mod expression;
mod rule;

pub(crate) use expression::{Expression, ExpressionKind};
pub(crate) use rule::{Rule, RuleId, reachable};

/// A grammar is its rules, with every reference between them resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Grammar {
    /// The rules in the order they are written, without the later definitions
    /// of a meta identifier.
    pub(crate) syntax_rules: Vec<Rule>,
    /// The rules no other rule references. Each gets its own parser.
    pub(crate) start_syntax_rules: Vec<RuleId>,
}

impl Grammar {
    /// Calls `visit` with every expression of every rule, each before the ones
    /// inside it.
    pub(crate) fn visit(&self, visit: &mut impl FnMut(&Expression)) {
        for rule in &self.syntax_rules {
            rule.expression.visit(visit);
        }
    }

    /// Returns which rules the rules in `starts` reach through references,
    /// those in `starts` included.
    pub(crate) fn reached_from(
        &self,
        starts: impl IntoIterator<Item = RuleId>,
    ) -> Vec<bool> {
        let rules = &self.syntax_rules;
        reachable(rules.len(), starts, |RuleId(position), pending| {
            rules[position].expression.visit_meta_identifiers(
                &mut |rule_id, _| pending.push(rule_id),
            );
        })
    }

    /// Returns which rules can reach themselves through references, so their
    /// matches can nest as deeply as an input does.
    pub(crate) fn recursive_rules(&self) -> Vec<bool> {
        (0..self.syntax_rules.len())
            .map(|position| {
                let mut read = Vec::new();
                self.syntax_rules[position]
                    .expression
                    .visit_meta_identifiers(&mut |rule_id, _| {
                        read.push(rule_id);
                    });
                self.reached_from(read)[position]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::dictionary::tests::build_valid;

    #[test]
    fn tells_the_rules_that_reach_themselves() {
        let grammar = build_valid(
            "s = a, b; a = '(', [a], ')'; b = c; c = 'x', [d]; d = c;",
        );
        assert_eq!(
            grammar.recursive_rules(),
            [false, true, false, true, true]
        );
    }
}
