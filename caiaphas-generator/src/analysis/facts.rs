//! Facts are what several checks need to know about a grammar. Each is
//! computed the first time a check asks for it.

mod expression;
mod follow;
#[cfg(test)]
mod tests;

use std::cell::OnceCell;

use expression::{
    SingleCharacterSearch, always_matches, first, inner, is_nullable,
    is_productive, rule_single_characters, single_characters,
};
pub(crate) use follow::Follow;

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
    /// Whether each rule matches wherever it is tried.
    always_matching: OnceCell<Vec<bool>>,
    /// Whether each rule can finish matching some input.
    productive: OnceCell<Vec<bool>>,
    /// The characters each rule can start with.
    first: OnceCell<Vec<CharacterSet>>,
    /// The characters the matches of each rule can hold after their first.
    inner: OnceCell<Vec<CharacterSet>>,
    /// What can come right after each rule.
    follow: OnceCell<Vec<Follow>>,
    /// The single characters of each rule.
    single_characters: OnceCell<Vec<Option<CharacterSet>>>,
}

impl<'a> Facts<'a> {
    pub(crate) fn new(grammar: &'a Grammar) -> Self {
        Self {
            grammar,
            nullable: OnceCell::new(),
            always_matching: OnceCell::new(),
            productive: OnceCell::new(),
            first: OnceCell::new(),
            inner: OnceCell::new(),
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

    /// Returns whether `expression` matches wherever it is tried, if only
    /// nothing.
    ///
    /// A syntactic term never counts as one, since its exception can take its
    /// match away.
    pub(crate) fn always_matches(&self, expression: &Expression) -> bool {
        always_matches(expression, self.always_matching_rules())
    }

    /// Returns whether some input can finish matching `expression`.
    ///
    /// A syntactic term counts as one when its factor does, even if its
    /// exception takes every match away.
    pub(crate) fn is_productive(&self, expression: &Expression) -> bool {
        is_productive(expression, self.productive_rules())
    }

    /// Returns the characters `expression` can start with.
    ///
    /// A syntactic term starts with what its factor starts with, and leaves
    /// out what its exception matches only when both only ever match one
    /// character.
    pub(crate) fn first(&self, expression: &Expression) -> CharacterSet {
        first(
            expression,
            self.nullable_rules(),
            self.first_rules(),
            self.single_character_rules(),
        )
    }

    /// Returns the characters a match of `expression` can hold after its first
    /// character.
    ///
    /// It counts the first character of every expression after the first of a
    /// sequence, even where those before it can match nothing, so it can only
    /// tell too many characters, never too few.
    pub(crate) fn inner(&self, expression: &Expression) -> CharacterSet {
        inner(expression, self, self.inner_rules())
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
        self.add_first_meta_identifiers(expression, &mut found);
        found
    }

    /// Returns the grammar the facts are about.
    pub(crate) fn grammar(&self) -> &'a Grammar {
        self.grammar
    }

    fn add_first_meta_identifiers(
        &self,
        expression: &Expression,
        found: &mut Vec<(RuleId, Span)>,
    ) {
        match &expression.kind {
            ExpressionKind::SingleDefinition(expressions) => {
                // The start of a sequence reaches past every expression that
                // can match nothing.
                for expression in expressions {
                    self.add_first_meta_identifiers(expression, found);
                    if !self.is_nullable(expression) {
                        break;
                    }
                }
            }
            ExpressionKind::DefinitionsList(expressions) => {
                for expression in expressions {
                    self.add_first_meta_identifiers(expression, found);
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
            } => self.add_first_meta_identifiers(expression, found),
            ExpressionKind::MetaIdentifier(rule_id) => {
                found.push((*rule_id, expression.span));
            }
            ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::EmptySequence => {}
        }
    }

    fn nullable_rules(&self) -> &[bool] {
        self.nullable
            .get_or_init(|| self.fixed_point(false, is_nullable))
    }

    fn always_matching_rules(&self) -> &[bool] {
        self.always_matching
            .get_or_init(|| self.fixed_point(false, always_matches))
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
                let next = value(&rule.expression, &values);
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

    fn inner_rules(&self) -> &[CharacterSet] {
        self.inner.get_or_init(|| {
            self.fixed_point(CharacterSet::default(), |expression, sets| {
                inner(expression, self, sets)
            })
        })
    }

    fn single_character_rules(&self) -> &[Option<CharacterSet>] {
        self.single_characters.get_or_init(|| {
            let rules = &self.grammar.syntax_rules;
            let mut states =
                vec![SingleCharacterSearch::Unvisited; rules.len()];
            (0..rules.len())
                .map(|position| {
                    rule_single_characters(rules, &mut states, position)
                })
                .collect()
        })
    }
}
