//! Facts are what several checks need to know about a grammar. Each is
//! computed the first time a check asks for it.

use std::cell::OnceCell;

use crate::{
    analysis::character_set::CharacterSet,
    ir::{Expression, ExpressionKind, Grammar, RuleId},
    span::Span,
};

/// Answers questions about the expressions of a grammar.
pub(crate) struct Facts<'a> {
    grammar: &'a Grammar,
    /// Whether each rule can match nothing.
    nullable: OnceCell<Vec<bool>>,
    /// Whether each rule can finish matching some input.
    productive: OnceCell<Vec<bool>>,
    /// The characters each rule can start with.
    first: OnceCell<Vec<CharacterSet>>,
    /// The characters that can come right after each rule.
    follow: OnceCell<Vec<CharacterSet>>,
}

impl<'a> Facts<'a> {
    pub(crate) fn new(grammar: &'a Grammar) -> Self {
        Self {
            grammar,
            nullable: OnceCell::new(),
            productive: OnceCell::new(),
            first: OnceCell::new(),
            follow: OnceCell::new(),
        }
    }

    /// Returns whether `expression` can match without reading anything.
    ///
    /// A syntactic term counts as one when its factor does, even if its
    /// exception takes the empty match away, so checks report too much rather
    /// than too little.
    pub(crate) fn is_nullable(&self, expression: &Expression) -> bool {
        is_nullable(expression, self.nullable_rules())
    }

    /// Returns whether some input can finish matching `expression`.
    ///
    /// A syntactic term counts as one when its factor does, even if its
    /// exception takes every match away.
    pub(crate) fn is_productive(&self, expression: &Expression) -> bool {
        is_productive(expression, self.productive_rules())
    }

    /// Returns the characters `expression` can start with.
    pub(crate) fn first(&self, expression: &Expression) -> CharacterSet {
        first(expression, self.nullable_rules(), self.first_rules())
    }

    /// Returns the characters `expression` can start with when `after` comes
    /// right after it.
    pub(crate) fn first_before(
        &self,
        expression: &Expression,
        after: &CharacterSet,
    ) -> CharacterSet {
        let first = self.first(expression);
        if self.is_nullable(expression) {
            first.union(after)
        } else {
            first
        }
    }

    /// Returns the meta identifiers `expression` can read before any
    /// character.
    pub(crate) fn first_meta_identifiers(
        &self,
        expression: &Expression,
    ) -> Vec<(RuleId, Span)> {
        let mut found = Vec::new();
        self.edge_meta_identifiers(expression, Edge::Start, &mut found);
        found
    }

    /// Returns the meta identifiers `expression` can read after its last
    /// character.
    pub(crate) fn last_meta_identifiers(
        &self,
        expression: &Expression,
    ) -> Vec<(RuleId, Span)> {
        let mut found = Vec::new();
        self.edge_meta_identifiers(expression, Edge::End, &mut found);
        found
    }

    /// Calls `visit` with every expression of every rule, and the characters
    /// that can come right after it.
    pub(crate) fn walk(
        &self,
        visit: &mut impl FnMut(&Expression, &CharacterSet),
    ) {
        for (position, rule) in self.grammar.syntax_rules.iter().enumerate() {
            let after = self.follow_rules()[position].clone();
            self.walk_expression(&rule.definitions_list, &after, visit);
        }
    }

    fn walk_expression(
        &self,
        expression: &Expression,
        after: &CharacterSet,
        visit: &mut impl FnMut(&Expression, &CharacterSet),
    ) {
        visit(expression, after);
        match &expression.kind {
            ExpressionKind::SingleDefinition(expressions) => {
                let mut rest = after.clone();
                for expression in expressions.iter().rev() {
                    self.walk_expression(expression, &rest, visit);
                    rest = self.first_before(expression, &rest);
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
                let again = self.first(expression).union(after);
                self.walk_expression(expression, &again, visit);
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => {
                self.walk_expression(syntactic_factor, after, visit);
                self.walk_expression(syntactic_exception, after, visit);
            }
            ExpressionKind::MetaIdentifier(_)
            | ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::EmptySequence => {}
        }
    }

    fn edge_meta_identifiers(
        &self,
        expression: &Expression,
        edge: Edge,
        found: &mut Vec<(RuleId, Span)>,
    ) {
        match &expression.kind {
            ExpressionKind::SingleDefinition(expressions) => {
                let ordered: Vec<&Expression> = match edge {
                    Edge::Start => expressions.iter().collect(),
                    Edge::End => expressions.iter().rev().collect(),
                };
                for expression in ordered {
                    self.edge_meta_identifiers(expression, edge, found);
                    if !self.is_nullable(expression) {
                        break;
                    }
                }
            }
            ExpressionKind::DefinitionsList(expressions) => {
                for expression in expressions {
                    self.edge_meta_identifiers(expression, edge, found);
                }
            }
            ExpressionKind::OptionalSequence(expression)
            | ExpressionKind::RepeatedSequence(expression)
            | ExpressionKind::SyntacticFactor {
                syntactic_primary: expression,
                ..
            }
            | ExpressionKind::SyntacticTerm {
                syntactic_factor: expression,
                ..
            } => self.edge_meta_identifiers(expression, edge, found),
            ExpressionKind::MetaIdentifier(rule_id) => {
                found.push((*rule_id, expression.span));
            }
            ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::EmptySequence => {}
        }
    }

    fn nullable_rules(&self) -> &[bool] {
        self.nullable.get_or_init(|| self.rules_where(is_nullable))
    }

    fn productive_rules(&self) -> &[bool] {
        self.productive
            .get_or_init(|| self.rules_where(is_productive))
    }

    /// Returns which rules `holds` is true for, where whether it holds for a
    /// rule depends on whether it holds for the rules it references.
    ///
    /// Starts from false for every rule and turns rules true until none
    /// changes, so recursion that never ends makes a rule false.
    fn rules_where(
        &self,
        holds: fn(&Expression, &[bool]) -> bool,
    ) -> Vec<bool> {
        let rules = &self.grammar.syntax_rules;
        let mut values = vec![false; rules.len()];
        let mut changed = true;
        while changed {
            changed = false;
            for (position, rule) in rules.iter().enumerate() {
                if !values[position] && holds(&rule.definitions_list, &values)
                {
                    values[position] = true;
                    changed = true;
                }
            }
        }
        values
    }

    fn first_rules(&self) -> &[CharacterSet] {
        self.first.get_or_init(|| {
            let rules = &self.grammar.syntax_rules;
            let nullable = self.nullable_rules();
            let mut sets = vec![CharacterSet::default(); rules.len()];
            let mut changed = true;
            while changed {
                changed = false;
                for (position, rule) in rules.iter().enumerate() {
                    let set = first(&rule.definitions_list, nullable, &sets);
                    if set != sets[position] {
                        sets[position] = set;
                        changed = true;
                    }
                }
            }
            sets
        })
    }

    fn follow_rules(&self) -> &[CharacterSet] {
        self.follow.get_or_init(|| {
            let rules = &self.grammar.syntax_rules;
            let mut sets = vec![CharacterSet::default(); rules.len()];
            let mut changed = true;
            while changed {
                changed = false;
                for (position, rule) in rules.iter().enumerate() {
                    let mut found = Vec::new();
                    self.walk_expression(
                        &rule.definitions_list,
                        &sets[position].clone(),
                        &mut |expression, after| {
                            if let ExpressionKind::MetaIdentifier(rule_id) =
                                expression.kind
                            {
                                found.push((rule_id, after.clone()));
                            }
                        },
                    );
                    for (RuleId(target), after) in found {
                        let set = sets[target].union(&after);
                        if set != sets[target] {
                            sets[target] = set;
                            changed = true;
                        }
                    }
                }
            }
            sets
        })
    }
}

/// Which end of an expression to look at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Start,
    End,
}

fn is_nullable(expression: &Expression, rules: &[bool]) -> bool {
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => expressions
            .iter()
            .all(|expression| is_nullable(expression, rules)),
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .any(|expression| is_nullable(expression, rules)),
        ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::EmptySequence => true,
        ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        }
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => is_nullable(expression, rules),
        ExpressionKind::MetaIdentifier(RuleId(position)) => rules[*position],
        ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_) => false,
    }
}

fn is_productive(expression: &Expression, rules: &[bool]) -> bool {
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => expressions
            .iter()
            .all(|expression| is_productive(expression, rules)),
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .any(|expression| is_productive(expression, rules)),
        ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_)
        | ExpressionKind::EmptySequence => true,
        ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        }
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => is_productive(expression, rules),
        ExpressionKind::MetaIdentifier(RuleId(position)) => rules[*position],
    }
}

fn first(
    expression: &Expression,
    nullable: &[bool],
    rules: &[CharacterSet],
) -> CharacterSet {
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => {
            let mut set = CharacterSet::default();
            for expression in expressions {
                set = set.union(&first(expression, nullable, rules));
                if !is_nullable(expression, nullable) {
                    break;
                }
            }
            set
        }
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .fold(CharacterSet::default(), |set, expression| {
                set.union(&first(expression, nullable, rules))
            }),
        ExpressionKind::OptionalSequence(expression)
        | ExpressionKind::RepeatedSequence(expression)
        | ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        }
        | ExpressionKind::SyntacticTerm {
            syntactic_factor: expression,
            ..
        } => first(expression, nullable, rules),
        ExpressionKind::MetaIdentifier(RuleId(position)) => {
            rules[*position].clone()
        }
        ExpressionKind::TerminalString(string) => string
            .chars()
            .next()
            .map_or_else(CharacterSet::default, CharacterSet::single),
        ExpressionKind::SpecialSequence(range) => {
            CharacterSet::from_range(range.clone())
        }
        ExpressionKind::EmptySequence => CharacterSet::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dictionary::build, parser::parse};

    fn grammar(source: &str) -> Grammar {
        build(&parse(source).0).0
    }

    fn rule(grammar: &Grammar, position: usize) -> &Expression {
        &grammar.syntax_rules[position].definitions_list
    }

    #[test]
    fn finds_rules_that_can_match_nothing() {
        let grammar = grammar("a = b, ['x']; b = {'y'} | c; c = 'z';");
        let facts = Facts::new(&grammar);
        let nullable: Vec<bool> = (0..3)
            .map(|position| facts.is_nullable(rule(&grammar, position)))
            .collect();
        assert_eq!(nullable, [true, true, false]);
    }

    #[test]
    fn finds_rules_that_can_finish_matching() {
        let grammar =
            grammar("s = a | b | c; a = 'x', a; b = a | 'y'; c = [c], 'z';");
        let facts = Facts::new(&grammar);
        let productive: Vec<bool> = (0..4)
            .map(|position| facts.is_productive(rule(&grammar, position)))
            .collect();
        assert_eq!(productive, [true, false, true, true]);
    }

    #[test]
    fn finds_what_rules_start_with_through_nullable_parts() {
        let grammar = grammar("a = ['x'], b, 'w'; b = {'y'}, 'z';");
        let facts = Facts::new(&grammar);
        assert_eq!(facts.first(rule(&grammar, 0)).to_string(), "'x'..='z'");
        assert_eq!(facts.first(rule(&grammar, 1)).to_string(), "'y'..='z'");
    }

    #[test]
    fn finds_what_follows_each_rule() {
        let grammar = grammar("a = b, 'x' | c; b = 'y'; c = b, [b], 'z';");
        let facts = Facts::new(&grammar);
        let follow =
            |position: usize| facts.follow_rules()[position].to_string();
        assert_eq!(follow(0), "");
        assert_eq!(follow(1), "'x'..='z'");
        assert_eq!(follow(2), "");
    }

    #[test]
    fn finds_meta_identifiers_at_either_edge() {
        let grammar = grammar("a = [b], c, 'x', [b], c; b = 'y'; c = 'z';");
        let facts = Facts::new(&grammar);
        let names = |found: Vec<(RuleId, Span)>| -> Vec<usize> {
            found
                .into_iter()
                .map(|(RuleId(position), _)| position)
                .collect()
        };
        assert_eq!(
            names(facts.first_meta_identifiers(rule(&grammar, 0))),
            [1, 2]
        );
        assert_eq!(names(facts.last_meta_identifiers(rule(&grammar, 0))), [2]);
    }

    #[test]
    fn walks_every_expression_with_what_follows_it() {
        let grammar = grammar("a = 'x', {'y'}, 'z';");
        let facts = Facts::new(&grammar);
        let mut seen = Vec::new();
        facts.walk(&mut |expression, after| {
            if let ExpressionKind::TerminalString(string) = &expression.kind {
                seen.push(format!("{string}:{after}"));
            }
        });
        seen.sort();
        assert_eq!(seen, ["x:'y'..='z'", "y:'y'..='z'", "z:"]);
    }
}
