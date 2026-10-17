//! The dictionary resolves the syntax rules of a grammar into its intermediate
//! representation.

mod inline_rules;
mod lower;
mod pattern;
mod start_rules;
mod syntactic_exception;

use std::{
    collections::{HashMap, hash_map::Entry},
    error::Error,
    fmt,
};

use crate::{
    diagnostic::{Diagnostic, Label, Severity},
    ir::{Grammar, RuleId},
    lexer::MetaIdentifier,
    parser::{Syntax, SyntaxRule},
    span::Span,
    spelling,
};

/// Returns the grammar `syntax` defines, and every error found while resolving
/// it.
///
/// The grammar holds every rule but the later definitions of a meta
/// identifier, even rules with errors, so later checks can still read the
/// rest.
pub(crate) fn build(syntax: &Syntax) -> (Grammar, Vec<DictionaryError>) {
    let mut errors = Vec::new();
    let index = Index::new(syntax, &mut errors);
    let syntax_rules = lower::lower(&index, &mut errors);
    let start_syntax_rules =
        start_rules::start_rules(&syntax_rules, &mut errors);
    syntactic_exception::check(&syntax_rules, &mut errors);
    inline_rules::check(&syntax_rules, &mut errors);
    errors.sort_by_key(|error| error.span.start);
    (
        Grammar {
            syntax_rules,
            start_syntax_rules,
        },
        errors,
    )
}

/// Maps each meta identifier to the syntax rule that defines it.
struct Index<'a> {
    /// The syntax rules in the order they are written, without those that
    /// define a meta identifier again.
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

    /// Returns whether `meta_identifier` is written as several words that each
    /// name a rule, as when the `,` between them is missing.
    fn separate(&self, meta_identifier: &MetaIdentifier) -> bool {
        meta_identifier.words().count() > 1
            && meta_identifier
                .words()
                .all(|word| self.rule_ids.contains_key(word))
    }

    /// Returns the defined meta identifier closest to `meta_identifier` when
    /// few enough edits make one into the other, as with a misspelling.
    fn similar(
        &self,
        meta_identifier: &MetaIdentifier,
    ) -> Option<MetaIdentifier> {
        let names = self
            .syntax_rules
            .iter()
            .map(|rule| rule.meta_identifier.as_str());
        let similar = spelling::closest(meta_identifier.as_str(), names)?;
        self.syntax_rules
            .iter()
            .map(|rule| &rule.meta_identifier)
            .find(|other| other.as_str() == similar)
            .cloned()
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
    /// No syntax rule defines a meta identifier that is used.
    UndefinedMetaIdentifier {
        meta_identifier: MetaIdentifier,
        /// A defined meta identifier written almost the same way, if any.
        similar: Option<MetaIdentifier>,
        /// Whether each of its words names a rule.
        separate: bool,
        /// Whether it is one letter of a term such as `a - z`, as a class of
        /// characters is written in a regular expression.
        class: bool,
    },
    /// A syntactic exception uses a meta identifier whose rule is recursive or
    /// references a recursive rule.
    RecursiveSyntacticException { meta_identifier: MetaIdentifier },
    /// A `(* hide *)` comes right before a start rule. Its parser has to
    /// return its type.
    HiddenStartSyntaxRule { meta_identifier: MetaIdentifier },
    /// An `(* as T *)` comes right before a start rule. Its parser has to be a
    /// method of its own type.
    ConvertedStartSyntaxRule { meta_identifier: MetaIdentifier },
    /// An `(* inline *)` comes right before a start rule. Its parser has to
    /// return its type.
    InlineStartSyntaxRule { meta_identifier: MetaIdentifier },
    /// An inline rule holds itself through inline rules alone.
    RecursiveInlineSyntaxRule { meta_identifier: MetaIdentifier },
}

impl fmt::Display for DictionaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DictionaryErrorKind::DuplicateDefinition {
                meta_identifier,
                ..
            } => {
                write!(f, "`{}` is already defined", meta_identifier.spaced())
            }
            DictionaryErrorKind::UndefinedMetaIdentifier {
                meta_identifier,
                ..
            } => {
                write!(f, "`{}` is not defined", meta_identifier.spaced())
            }
            DictionaryErrorKind::RecursiveSyntacticException {
                meta_identifier,
            } => write!(
                f,
                "`{}` is or references a recursive rule, so a syntactic \
                 exception cannot use it",
                meta_identifier.spaced()
            ),
            DictionaryErrorKind::HiddenStartSyntaxRule { meta_identifier } => {
                write!(
                    f,
                    "`{}` is a start rule, so it cannot be hidden",
                    meta_identifier.spaced()
                )
            }
            DictionaryErrorKind::ConvertedStartSyntaxRule {
                meta_identifier,
            } => write!(
                f,
                "`{}` is a start rule, so its type is its own, not one its \
                 text converts to",
                meta_identifier.spaced()
            ),
            DictionaryErrorKind::InlineStartSyntaxRule { meta_identifier } => {
                write!(
                    f,
                    "`{}` is a start rule, so it cannot be inline",
                    meta_identifier.spaced()
                )
            }
            DictionaryErrorKind::RecursiveInlineSyntaxRule {
                meta_identifier,
            } => write!(
                f,
                "`{}` holds itself through inline rules alone, so what it \
                 gives the rules that hold it would never end",
                meta_identifier.spaced()
            ),
        }
    }
}

impl Error for DictionaryError {}

impl DictionaryError {
    /// Returns how to fix the grammar.
    fn help(&self) -> String {
        match &self.kind {
            DictionaryErrorKind::DuplicateDefinition { .. } => {
                "join both definitions into one rule with `|`, or rename one \
                 of them"
                    .to_owned()
            }
            DictionaryErrorKind::UndefinedMetaIdentifier {
                meta_identifier,
                separate: true,
                ..
            } => {
                let words: Vec<&str> = meta_identifier.words().collect();
                format!(
                    "the gaps join `{}` into one name; to read each rule in \
                     turn, write `{}`",
                    meta_identifier.written(),
                    words.join(", ")
                )
            }
            DictionaryErrorKind::UndefinedMetaIdentifier {
                meta_identifier,
                similar: Some(similar),
                ..
            } => format!(
                "correct its spelling to `{}`, or define `{}` with a syntax \
                 rule",
                similar.spaced(),
                meta_identifier.spaced()
            ),
            DictionaryErrorKind::UndefinedMetaIdentifier {
                meta_identifier,
                class: true,
                ..
            } => format!(
                "define `{}` with a syntax rule; for a class of characters, \
                 as `[a-z]` is in a regular expression, write `? /[a-z]/ ?`",
                meta_identifier.spaced()
            ),
            DictionaryErrorKind::UndefinedMetaIdentifier {
                meta_identifier,
                similar: None,
                ..
            } => {
                format!(
                    "define `{}` with a syntax rule, or correct its spelling",
                    meta_identifier.spaced()
                )
            }
            DictionaryErrorKind::RecursiveSyntacticException {
                meta_identifier,
            } => format!(
                "write the exception with terminal strings and special \
                 sequences instead of `{}`",
                meta_identifier.spaced()
            ),
            DictionaryErrorKind::HiddenStartSyntaxRule { .. } => {
                "remove the `(* hide *)` before it, since its parser returns \
                 its type"
                    .to_owned()
            }
            DictionaryErrorKind::ConvertedStartSyntaxRule { .. } => {
                "remove the `(* as ... *)` before it, since its parser is a \
                 method of its own type"
                    .to_owned()
            }
            DictionaryErrorKind::InlineStartSyntaxRule { .. } => {
                "remove the `(* inline *)` before it, since its parser \
                 returns its type"
                    .to_owned()
            }
            DictionaryErrorKind::RecursiveInlineSyntaxRule { .. } => {
                "remove the `(* inline *)` before one of those rules, so its \
                 type holds the rest"
                    .to_owned()
            }
        }
    }
}

impl From<DictionaryError> for Diagnostic {
    fn from(error: DictionaryError) -> Self {
        let labels = match &error.kind {
            DictionaryErrorKind::DuplicateDefinition {
                first_definition,
                ..
            } => vec![Label {
                span: *first_definition,
                message: "first defined here",
            }],
            _ => Vec::new(),
        };
        Self {
            severity: Severity::Error,
            code: None,
            message: error.to_string(),
            span: error.span,
            labels,
            help: Some(error.help()),
            note: None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        ir::{Expression, ExpressionKind},
        parser::{parse, tests::parse_valid},
    };

    /// Returns the grammar `source` defines, and the dictionary errors in it.
    pub(super) fn grammar(source: &str) -> (Grammar, Vec<DictionaryError>) {
        build(&parse_valid(source))
    }

    /// Returns the grammar `source` defines, asserting it has no errors.
    pub(crate) fn build_valid(source: &str) -> Grammar {
        let (grammar, errors) = grammar(source);
        assert_eq!(errors, [], "{source:?}");
        grammar
    }

    /// Writes the rule `name` defines as a compact expression, such as
    /// `seq(alt('x', 'y'), opt(b))`.
    pub(crate) fn render(grammar: &Grammar, name: &str) -> String {
        let rule = grammar
            .syntax_rules
            .iter()
            .find(|rule| rule.meta_identifier.as_str() == name)
            .expect("the grammar defines the rule");
        render_expression(grammar, &rule.expression)
    }

    fn render_expression(
        grammar: &Grammar,
        expression: &Expression,
    ) -> String {
        let list = |name: &str, expressions: &[Expression]| {
            let items: Vec<String> = expressions
                .iter()
                .map(|expression| render_expression(grammar, expression))
                .collect();
            format!("{name}({})", items.join(", "))
        };
        match &expression.kind {
            ExpressionKind::SingleDefinition(expressions) => {
                list("seq", expressions)
            }
            ExpressionKind::DefinitionsList(expressions) => {
                list("alt", expressions)
            }
            ExpressionKind::OptionalSequence(expression) => {
                format!("opt({})", render_expression(grammar, expression))
            }
            ExpressionKind::RepeatedSequence(expression) => {
                format!("rep({})", render_expression(grammar, expression))
            }
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => format!(
                "{integer}*{}",
                render_expression(grammar, syntactic_primary)
            ),
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => format!(
                "except({}, {})",
                render_expression(grammar, syntactic_factor),
                render_expression(grammar, syntactic_exception)
            ),
            ExpressionKind::MetaIdentifier(RuleId(position)) => grammar
                .syntax_rules[*position]
                .meta_identifier
                .as_str()
                .to_owned(),
            ExpressionKind::TerminalString(string) => format!("'{string}'"),
            ExpressionKind::SpecialSequence(range) => format!(
                "U+{:04X}..U+{:04X}",
                u32::from(*range.start()),
                u32::from(*range.end())
            ),
            ExpressionKind::EmptySequence => "empty".to_owned(),
        }
    }

    /// Returns the names of the start rules of `grammar`.
    pub(super) fn start_names(grammar: &Grammar) -> Vec<&str> {
        grammar
            .start_syntax_rules
            .iter()
            .map(|&RuleId(position)| {
                grammar.syntax_rules[position].meta_identifier.as_str()
            })
            .collect()
    }

    #[test]
    fn builds_a_grammar_without_errors() {
        let (grammar, errors) = grammar("a = b, 'x'; b = 'y' | 'z';");
        assert_eq!(errors, []);
        assert_eq!(render(&grammar, "a"), "seq(b, 'x')");
        assert_eq!(render(&grammar, "b"), "alt('y', 'z')");
        assert_eq!(start_names(&grammar), ["a"]);
    }

    #[test]
    fn collects_every_error_in_order() {
        let (_, errors) = grammar("a = c; a = 'x'; b = 'y' - b, [b];");
        let kinds: Vec<&str> = errors
            .iter()
            .map(|error| match error.kind {
                DictionaryErrorKind::DuplicateDefinition { .. } => "duplicate",
                DictionaryErrorKind::UndefinedMetaIdentifier { .. } => {
                    "undefined"
                }
                DictionaryErrorKind::RecursiveSyntacticException {
                    ..
                } => "recursive",
                DictionaryErrorKind::HiddenStartSyntaxRule { .. } => "hidden",
                DictionaryErrorKind::InlineStartSyntaxRule { .. } => "inline",
                DictionaryErrorKind::ConvertedStartSyntaxRule { .. } => {
                    "converted"
                }
                DictionaryErrorKind::RecursiveInlineSyntaxRule { .. } => {
                    "recursive inline"
                }
            })
            .collect();
        assert_eq!(kinds, ["undefined", "duplicate", "recursive"]);
    }

    #[test]
    fn keeps_every_rule_after_an_error() {
        let (grammar, _) = grammar("a = c; b = 'x';");
        assert_eq!(render(&grammar, "a"), "empty");
        assert_eq!(render(&grammar, "b"), "'x'");
    }

    #[test]
    fn builds_an_empty_grammar_without_errors() {
        let (grammar, errors) = build(&parse("").0);
        assert_eq!(grammar.syntax_rules, []);
        assert_eq!(errors, []);
    }

    #[test]
    fn describes_every_error_with_its_help() {
        let cases = [
            (
                "a = 'x'; a = 'y';",
                "`a` is already defined",
                "join both definitions into one rule with `|`, or rename one \
                 of them",
            ),
            (
                "a = b;",
                "`b` is not defined",
                "define `b` with a syntax rule, or correct its spelling",
            ),
            (
                "a = syntax  rule;",
                "`syntax rule` is not defined",
                "define `syntax rule` with a syntax rule, or correct its \
                 spelling",
            ),
            (
                "s = 'x' - b; b = 'y', [b];",
                "`b` is or references a recursive rule, so a syntactic \
                 exception cannot use it",
                "write the exception with terminal strings and special \
                 sequences instead of `b`",
            ),
            (
                "(* hide *) s = 'x';",
                "`s` is a start rule, so it cannot be hidden",
                "remove the `(* hide *)` before it, since its parser returns \
                 its type",
            ),
        ];
        for (source, message, help) in cases {
            let (_, errors) = grammar(source);
            let diagnostic = Diagnostic::from(errors[0].clone());
            assert_eq!(diagnostic.severity, Severity::Error, "{source:?}");
            assert_eq!(diagnostic.message, message, "{source:?}");
            assert_eq!(diagnostic.span, errors[0].span, "{source:?}");
            assert_eq!(diagnostic.help.as_deref(), Some(help), "{source:?}");
        }
    }

    #[test]
    fn points_at_the_first_definition_of_a_duplicate() {
        let (_, errors) = grammar("a = 'x'; a = 'y';");
        assert_eq!(
            Diagnostic::from(errors[0].clone()).labels,
            [Label {
                span: Span { start: 0, end: 8 },
                message: "first defined here",
            }]
        );
        let (_, errors) = grammar("a = b;");
        assert_eq!(Diagnostic::from(errors[0].clone()).labels, []);
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
        let syntax = parse_valid("a = b; b = 'x'; c = a;");
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
        let defined = parse_valid("a = b;");
        let other = parse_valid("c = d;");
        let index = Index::new(&defined, &mut Vec::new());
        assert_eq!(index.get(&other.syntax_rules[0].meta_identifier), None);
    }

    #[test]
    fn resolves_a_meta_identifier_written_with_other_gaps() {
        let grammar = build_valid("s = syntaxrule; syntax rule = 'x';");
        assert_eq!(render(&grammar, "s"), "syntaxrule");
    }

    #[test]
    fn rejects_a_meta_identifier_defined_twice() {
        let source = "a = 'x'; b = 'y'; a = 'z';";
        let syntax = parse_valid(source);
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
        let syntax = parse_valid("a = 'x'; a = 'y'; b = 'z';");
        let index = Index::new(&syntax, &mut Vec::new());
        assert_eq!(
            index.get(&syntax.syntax_rules[2].meta_identifier),
            Some(RuleId(1))
        );
    }

    #[test]
    fn rejects_every_extra_definition() {
        let syntax = parse_valid("a = 'x'; a = 'y'; a = 'z';");
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
}
