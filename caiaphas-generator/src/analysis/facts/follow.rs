//! The walk through a rule gives each of its expressions what can come right
//! after it, worked out from the rules that reference the rule.

use super::Facts;
use crate::{
    analysis::character_set::CharacterSet,
    ir::{Expression, ExpressionKind, RuleId},
};

/// What can come right after an expression.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Follow {
    pub(crate) characters: CharacterSet,
    /// Whether the input can end right after it, since a parser reads the
    /// whole input from a start rule.
    pub(crate) end_of_input: bool,
}

impl Follow {
    fn union(&self, other: &Self) -> Self {
        Self {
            characters: self.characters.union(&other.characters),
            end_of_input: self.end_of_input || other.end_of_input,
        }
    }
}

impl<'a> Facts<'a> {
    /// Calls `visit` with every expression of the rule `rule_id`, and what can
    /// come right after it.
    pub(crate) fn walk_rule(
        &self,
        rule_id: RuleId,
        visit: &mut impl FnMut(&'a Expression, &Follow),
    ) {
        let RuleId(position) = rule_id;
        let grammar = self.grammar;
        let after = &self.follow_rules()[position];
        self.walk_expression(
            &grammar.syntax_rules[position].expression,
            after,
            visit,
        );
    }

    /// Returns what can come right after the rule `rule_id`.
    pub(crate) fn follow(&self, rule_id: RuleId) -> &Follow {
        let RuleId(position) = rule_id;
        &self.follow_rules()[position]
    }

    fn walk_expression(
        &self,
        expression: &'a Expression,
        after: &Follow,
        visit: &mut impl FnMut(&'a Expression, &Follow),
    ) {
        visit(expression, after);
        match &expression.kind {
            ExpressionKind::SingleDefinition(expressions) => {
                let mut rest = after.clone();
                for expression in expressions.iter().rev() {
                    self.walk_expression(expression, &rest, visit);
                    rest = Follow {
                        characters: self
                            .first_before(expression, &rest.characters),
                        end_of_input: rest.end_of_input
                            && self.is_nullable(expression),
                    };
                }
            }
            ExpressionKind::DefinitionsList(expressions) => {
                for expression in expressions {
                    self.walk_expression(expression, after, visit);
                }
            }
            ExpressionKind::OptionalSequence(expression) => {
                self.walk_expression(expression, after, visit);
            }
            ExpressionKind::RepeatedSequence(expression)
            | ExpressionKind::SyntacticFactor {
                syntactic_primary: expression,
                ..
            } => {
                let again = Follow {
                    characters: self
                        .first(expression)
                        .union(&after.characters),
                    end_of_input: after.end_of_input,
                };
                self.walk_expression(expression, &again, visit);
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => {
                self.walk_expression(syntactic_factor, after, visit);
                // The exception only reads what its factor matched, so the
                // input it reads could end right after it.
                let exception_after = Follow {
                    characters: after.characters.clone(),
                    end_of_input: true,
                };
                self.walk_expression(
                    syntactic_exception,
                    &exception_after,
                    visit,
                );
            }
            ExpressionKind::MetaIdentifier(_)
            | ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::Not(_)
            | ExpressionKind::EmptySequence => {}
        }
    }

    fn follow_rules(&self) -> &[Follow] {
        self.follow.get_or_init(|| {
            let rules = &self.grammar.syntax_rules;
            let mut sets = vec![Follow::default(); rules.len()];
            for RuleId(position) in &self.grammar.start_syntax_rules {
                sets[*position].end_of_input = true;
            }
            let mut changed = true;
            while changed {
                changed = false;
                for (position, rule) in rules.iter().enumerate() {
                    let rule_after = sets[position].clone();
                    self.walk_expression(
                        &rule.expression,
                        &rule_after,
                        &mut |expression, after| {
                            let ExpressionKind::MetaIdentifier(rule_id) =
                                expression.kind
                            else {
                                return;
                            };
                            let RuleId(target) = rule_id;
                            let set = sets[target].union(after);
                            if set != sets[target] {
                                sets[target] = set;
                                changed = true;
                            }
                        },
                    );
                }
            }
            sets
        })
    }
}
