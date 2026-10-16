//! Facts are what several checks need to know about a grammar. Each is
//! computed the first time a check asks for it.

use std::cell::OnceCell;

use crate::{
    analysis::character_set::CharacterSet,
    ir::{Expression, ExpressionKind, Grammar, Rule, RuleId},
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
    /// What can come right after each rule.
    follow: OnceCell<Vec<Follow>>,
    /// The characters each rule matches, when it only ever matches one
    /// character.
    single_characters: OnceCell<Vec<Option<CharacterSet>>>,
}

impl<'a> Facts<'a> {
    pub(crate) fn new(grammar: &'a Grammar) -> Self {
        Self {
            grammar,
            nullable: OnceCell::new(),
            productive: OnceCell::new(),
            first: OnceCell::new(),
            follow: OnceCell::new(),
            single_characters: OnceCell::new(),
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
        first(
            expression,
            self.nullable_rules(),
            self.first_rules(),
            self.single_character_rules(),
        )
    }

    /// Returns the characters `expression` matches, when it only ever matches
    /// one character.
    ///
    /// A syntactic term matches exactly the characters of its factor that its
    /// exception does not, when both only ever match one character.
    pub(crate) fn single_characters(
        &self,
        expression: &Expression,
    ) -> Option<CharacterSet> {
        single_characters(expression, self.single_character_rules())
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

    /// Calls `visit` with every expression of every rule, and what can come
    /// right after it.
    pub(crate) fn walk(&self, visit: &mut impl FnMut(&Expression, &Follow)) {
        for (position, rule) in self.grammar.syntax_rules.iter().enumerate() {
            let after = &self.follow_rules()[position];
            self.walk_expression(&rule.definitions_list, after, visit);
        }
    }

    fn walk_expression(
        &self,
        expression: &Expression,
        after: &Follow,
        visit: &mut impl FnMut(&Expression, &Follow),
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
                // The exception only reads what its factor matched, so it ends
                // where that does, as if the input ended there.
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
            ExpressionKind::SingleDefinition(expressions) => match edge {
                Edge::Start => {
                    self.sequence_edge(expressions.iter(), edge, found);
                }
                Edge::End => {
                    self.sequence_edge(expressions.iter().rev(), edge, found);
                }
            },
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

    /// Adds the meta identifiers at `edge` of a sequence, whose `expressions`
    /// come from that edge inward. The edge reaches past every expression that
    /// can match nothing.
    fn sequence_edge<'e>(
        &self,
        expressions: impl Iterator<Item = &'e Expression>,
        edge: Edge,
        found: &mut Vec<(RuleId, Span)>,
    ) {
        for expression in expressions {
            self.edge_meta_identifiers(expression, edge, found);
            if !self.is_nullable(expression) {
                break;
            }
        }
    }

    fn nullable_rules(&self) -> &[bool] {
        self.nullable
            .get_or_init(|| self.fixed_point(false, is_nullable))
    }

    fn productive_rules(&self) -> &[bool] {
        self.productive
            .get_or_init(|| self.fixed_point(false, is_productive))
    }

    /// Returns a value for each rule, where the value of a rule depends on the
    /// values of the rules it references.
    ///
    /// Starts every rule at `initial` and works each out again with `value`
    /// until none changes, so recursion that never ends adds nothing.
    fn fixed_point<T: Clone + PartialEq>(
        &self,
        initial: T,
        value: impl Fn(&Expression, &[T]) -> T,
    ) -> Vec<T> {
        let rules = &self.grammar.syntax_rules;
        let mut values = vec![initial; rules.len()];
        let mut changed = true;
        while changed {
            changed = false;
            for (position, rule) in rules.iter().enumerate() {
                let next = value(&rule.definitions_list, &values);
                if next != values[position] {
                    values[position] = next;
                    changed = true;
                }
            }
        }
        values
    }

    fn first_rules(&self) -> &[CharacterSet] {
        self.first.get_or_init(|| {
            let nullable = self.nullable_rules();
            let characters = self.single_character_rules();
            self.fixed_point(CharacterSet::default(), |expression, sets| {
                first(expression, nullable, sets, characters)
            })
        })
    }

    fn single_character_rules(&self) -> &[Option<CharacterSet>] {
        self.single_characters.get_or_init(|| {
            let rules = &self.grammar.syntax_rules;
            let mut states = vec![Characters::Unvisited; rules.len()];
            (0..rules.len())
                .map(|position| rule_characters(rules, &mut states, position))
                .collect()
        })
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
                        &rule.definitions_list,
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

/// What can come right after an expression.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Follow {
    pub(crate) characters: CharacterSet,
    /// Whether the input can end right after it, since a parser reads the
    /// whole input from a start rule. The end of what an exception reads
    /// counts too, since the exception only reads what its factor matched.
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
    characters: &[Option<CharacterSet>],
) -> CharacterSet {
    let first = |expression| first(expression, nullable, rules, characters);
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => {
            let mut set = CharacterSet::default();
            for expression in expressions {
                set = set.union(&first(expression));
                if !is_nullable(expression, nullable) {
                    break;
                }
            }
            set
        }
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .fold(CharacterSet::default(), |set, expression| {
                set.union(&first(expression))
            }),
        ExpressionKind::OptionalSequence(expression)
        | ExpressionKind::RepeatedSequence(expression)
        | ExpressionKind::SyntacticFactor {
            syntactic_primary: expression,
            ..
        } => first(expression),
        ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } => match (
            single_characters(syntactic_factor, characters),
            single_characters(syntactic_exception, characters),
        ) {
            (Some(factor), Some(exception)) => factor.difference(&exception),
            _ => first(syntactic_factor),
        },
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

/// What is known so far about the characters a rule matches.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Characters {
    Unvisited,
    /// Being worked out, so reaching it again means recursion.
    Visiting,
    Known(Option<CharacterSet>),
}

/// Returns the characters the rule at `position` matches when it only ever
/// matches one character, working out the rules it references first.
///
/// A recursive rule gets no answer.
fn rule_characters(
    rules: &[Rule],
    states: &mut [Characters],
    position: usize,
) -> Option<CharacterSet> {
    match &states[position] {
        Characters::Known(characters) => return characters.clone(),
        Characters::Visiting => return None,
        Characters::Unvisited => {}
    }
    states[position] = Characters::Visiting;
    let characters = single_characters_with(
        &rules[position].definitions_list,
        &mut |RuleId(target)| rule_characters(rules, states, target),
    );
    states[position] = Characters::Known(characters.clone());
    characters
}

/// Returns the characters `expression` matches when it only ever matches one
/// character, given the same for each rule in `characters`.
fn single_characters(
    expression: &Expression,
    characters: &[Option<CharacterSet>],
) -> Option<CharacterSet> {
    single_characters_with(expression, &mut |RuleId(position)| {
        characters[position].clone()
    })
}

fn single_characters_with(
    expression: &Expression,
    rule: &mut impl FnMut(RuleId) -> Option<CharacterSet>,
) -> Option<CharacterSet> {
    match &expression.kind {
        ExpressionKind::TerminalString(string) => {
            let mut chars = string.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(CharacterSet::single(c)),
                _ => None,
            }
        }
        ExpressionKind::SpecialSequence(range) => {
            Some(CharacterSet::from_range(range.clone()))
        }
        ExpressionKind::DefinitionsList(expressions) => {
            let mut set = CharacterSet::default();
            for expression in expressions {
                set = set.union(&single_characters_with(expression, rule)?);
            }
            Some(set)
        }
        ExpressionKind::MetaIdentifier(rule_id) => rule(*rule_id),
        ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } => {
            let factor = single_characters_with(syntactic_factor, rule)?;
            let exception = single_characters_with(syntactic_exception, rule)?;
            Some(factor.difference(&exception))
        }
        ExpressionKind::SingleDefinition(_)
        | ExpressionKind::OptionalSequence(_)
        | ExpressionKind::RepeatedSequence(_)
        | ExpressionKind::SyntacticFactor { .. }
        | ExpressionKind::EmptySequence => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::build_valid;

    fn rule(grammar: &Grammar, position: usize) -> &Expression {
        &grammar.syntax_rules[position].definitions_list
    }

    #[test]
    fn finds_rules_that_can_match_nothing() {
        let grammar = build_valid("a = b, ['x']; b = {'y'} | c; c = 'z';");
        let facts = Facts::new(&grammar);
        let nullable: Vec<bool> = (0..3)
            .map(|position| facts.is_nullable(rule(&grammar, position)))
            .collect();
        assert_eq!(nullable, [true, true, false]);
    }

    #[test]
    fn finds_rules_that_can_finish_matching() {
        let grammar = build_valid(
            "s = a | b | c; a = 'x', a; b = a | 'y'; c = [c], 'z';",
        );
        let facts = Facts::new(&grammar);
        let productive: Vec<bool> = (0..4)
            .map(|position| facts.is_productive(rule(&grammar, position)))
            .collect();
        assert_eq!(productive, [true, false, true, true]);
    }

    #[test]
    fn finds_rules_that_only_match_single_characters() {
        let grammar = build_valid(
            "s = a | b | c | d | e; a = 'x' | ? U+0061..U+0063 ?; \
             b = 'xy'; c = a - 'b'; d = a, 'x'; e = 'x' | e;",
        );
        let facts = Facts::new(&grammar);
        let characters: Vec<Option<String>> = (0..6)
            .map(|position| {
                facts
                    .single_characters(rule(&grammar, position))
                    .map(|set| set.to_string())
            })
            .collect();
        assert_eq!(
            characters,
            [
                None,
                Some("'a'..='c', 'x'".to_owned()),
                None,
                Some("'a', 'c', 'x'".to_owned()),
                None,
                None,
            ]
        );
    }

    #[test]
    fn starts_a_term_with_what_its_exception_leaves() {
        let grammar = build_valid("s = ? U+0061..U+007A ? - ('x' | 'b');");
        let facts = Facts::new(&grammar);
        assert_eq!(
            facts.first(rule(&grammar, 0)).to_string(),
            "'a', 'c'..='w', 'y'..='z'"
        );
    }

    #[test]
    fn finds_what_rules_start_with_through_nullable_parts() {
        let grammar = build_valid("a = ['x'], b, 'w'; b = {'y'}, 'z';");
        let facts = Facts::new(&grammar);
        assert_eq!(facts.first(rule(&grammar, 0)).to_string(), "'x'..='z'");
        assert_eq!(facts.first(rule(&grammar, 1)).to_string(), "'y'..='z'");
    }

    #[test]
    fn finds_what_follows_each_rule() {
        let grammar = build_valid("a = b, 'x' | c; b = 'y'; c = b, [b], 'z';");
        let facts = Facts::new(&grammar);
        let follow = |position: usize| {
            facts.follow_rules()[position].characters.to_string()
        };
        assert_eq!(follow(0), "");
        assert_eq!(follow(1), "'x'..='z'");
        assert_eq!(follow(2), "");
        let ends: Vec<bool> = facts
            .follow_rules()
            .iter()
            .map(|follow| follow.end_of_input)
            .collect();
        assert_eq!(ends, [true, false, true]);
    }

    #[test]
    fn finds_meta_identifiers_at_either_edge() {
        let grammar =
            build_valid("a = [b], c, 'x', [b], c; b = 'y'; c = 'z';");
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
        let grammar = build_valid("a = 'x' - 'u', {'y'}, 'z', {'w'};");
        let facts = Facts::new(&grammar);
        let mut seen = Vec::new();
        facts.walk(&mut |expression, after| {
            if let ExpressionKind::TerminalString(string) = &expression.kind {
                let end = if after.end_of_input { " end" } else { "" };
                seen.push(format!("{string}:{}{end}", after.characters));
            }
        });
        seen.sort();
        assert_eq!(
            seen,
            [
                "u:'y'..='z' end",
                "w:'w' end",
                "x:'y'..='z'",
                "y:'y'..='z'",
                "z:'w' end"
            ]
        );
    }
}
