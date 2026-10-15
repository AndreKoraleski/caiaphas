//! The dictionary resolves the syntax rules of a grammar into its intermediate
//! representation.

use std::{
    collections::{HashMap, hash_map::Entry},
    error::Error,
    fmt,
};

use crate::{
    ir::RuleId,
    lexer::MetaIdentifier,
    parser::{Syntax, SyntaxRule},
    span::Span,
};

/// Maps each meta identifier to the syntax rule that defines it.
struct Index<'a> {
    /// The syntax rules in the order they are written, without those that
    /// define a meta identifier again, so a [`RuleId`] is an index.
    syntax_rules: Vec<&'a SyntaxRule>,
    rule_ids: HashMap<&'a str, RuleId>,
}

impl<'a> Index<'a> {
    /// Indexes the syntax rules of `syntax`, and reports every syntax rule
    /// that defines a meta identifier already defined.
    fn new(syntax: &'a Syntax, errors: &mut Vec<DictionaryError>) -> Self {
        let mut syntax_rules: Vec<&SyntaxRule> = Vec::new();
        let mut rule_ids = HashMap::new();
        for syntax_rule in &syntax.syntax_rules {
            let name = syntax_rule.meta_identifier.as_str();
            match rule_ids.entry(name) {
                Entry::Occupied(entry) => {
                    let RuleId(first) = *entry.get();
                    errors.push(DictionaryError {
                        kind: DictionaryErrorKind::DuplicateDefinition {
                            meta_identifier: syntax_rule
                                .meta_identifier
                                .clone(),
                            first_definition: syntax_rules[first].span,
                        },
                        span: syntax_rule.span,
                    });
                }
                Entry::Vacant(entry) => {
                    entry.insert(RuleId(syntax_rules.len()));
                    syntax_rules.push(syntax_rule);
                }
            }
        }
        Self {
            syntax_rules,
            rule_ids,
        }
    }

    /// Returns the rule that defines `meta_identifier`, if any.
    fn get(&self, meta_identifier: &MetaIdentifier) -> Option<RuleId> {
        self.rule_ids.get(meta_identifier.as_str()).copied()
    }
}

/// Error returned when the syntax rules of a grammar do not resolve into its
/// intermediate representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DictionaryError {
    pub(crate) kind: DictionaryErrorKind,
    pub(crate) span: Span,
}

/// What went wrong in a [`DictionaryError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DictionaryErrorKind {
    /// A syntax rule defines a meta identifier that an earlier one defines.
    DuplicateDefinition {
        meta_identifier: MetaIdentifier,
        first_definition: Span,
    },
}

impl fmt::Display for DictionaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DictionaryErrorKind::DuplicateDefinition {
                meta_identifier,
                ..
            } => {
                write!(f, "`{}` is already defined", meta_identifier.as_str())
            }
        }
    }
}

impl Error for DictionaryError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn syntax(source: &str) -> Syntax {
        let (syntax, errors) = parse(source);
        assert_eq!(errors, [], "{source:?}");
        syntax
    }

    fn names<'a>(index: &Index<'a>) -> Vec<&'a str> {
        index
            .syntax_rules
            .iter()
            .map(|syntax_rule| syntax_rule.meta_identifier.as_str())
            .collect()
    }

    #[test]
    fn indexes_every_syntax_rule_in_order() {
        let syntax = syntax("a = b; b = 'x'; c = a;");
        let mut errors = Vec::new();
        let index = Index::new(&syntax, &mut errors);
        assert_eq!(errors, []);
        assert_eq!(names(&index), ["a", "b", "c"]);
        for (position, syntax_rule) in syntax.syntax_rules.iter().enumerate() {
            assert_eq!(
                index.get(&syntax_rule.meta_identifier),
                Some(RuleId(position))
            );
        }
    }

    #[test]
    fn finds_nothing_for_an_undefined_meta_identifier() {
        let defined = syntax("a = b;");
        let other = syntax("c = d;");
        let index = Index::new(&defined, &mut Vec::new());
        assert_eq!(index.get(&other.syntax_rules[0].meta_identifier), None);
    }

    #[test]
    fn matches_meta_identifiers_without_their_gaps() {
        let syntax = syntax("syntax rule = 'x'; a = syntaxrule;");
        let index = Index::new(&syntax, &mut Vec::new());
        assert_eq!(names(&index), ["syntaxrule", "a"]);
    }

    #[test]
    fn rejects_a_meta_identifier_defined_twice() {
        let source = "a = 'x'; b = 'y'; a = 'z';";
        let syntax = syntax(source);
        let mut errors = Vec::new();
        let index = Index::new(&syntax, &mut errors);
        assert_eq!(names(&index), ["a", "b"]);
        assert_eq!(
            errors,
            [DictionaryError {
                kind: DictionaryErrorKind::DuplicateDefinition {
                    meta_identifier: syntax.syntax_rules[2]
                        .meta_identifier
                        .clone(),
                    first_definition: Span { start: 0, end: 8 },
                },
                span: Span { start: 18, end: 26 },
            }]
        );
    }

    #[test]
    fn keeps_rule_ids_in_order_after_a_duplicate() {
        let syntax = syntax("a = 'x'; a = 'y'; b = 'z';");
        let index = Index::new(&syntax, &mut Vec::new());
        assert_eq!(
            index.get(&syntax.syntax_rules[2].meta_identifier),
            Some(RuleId(1))
        );
    }

    #[test]
    fn rejects_every_extra_definition() {
        let syntax = syntax("a = 'x'; a = 'y'; a = 'z';");
        let mut errors = Vec::new();
        Index::new(&syntax, &mut errors);
        assert_eq!(errors.len(), 2);
        for error in &errors {
            assert!(matches!(
                error.kind,
                DictionaryErrorKind::DuplicateDefinition {
                    first_definition: Span { start: 0, end: 8 },
                    ..
                }
            ));
        }
    }

    #[test]
    fn error_displays_message() {
        let syntax = syntax("digit = '0'; digit = '1';");
        let mut errors = Vec::new();
        Index::new(&syntax, &mut errors);
        assert_eq!(errors[0].to_string(), "`digit` is already defined");
    }
}
