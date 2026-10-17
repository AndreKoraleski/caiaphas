//! Generating turns a grammar into the Rust code of its parser and of the
//! types of its rules.

mod code;
mod exact;
mod matching;
mod name;
#[doc(hidden)]
pub mod plan;
mod rearrange;
mod shape;

pub(crate) use shape::{gatherings, untraced_choices};

use std::collections::{HashMap, hash_map::Entry};

use proc_macro2::TokenStream;
use shape::Shape;

use crate::{
    compile::compile,
    diagnostic::{Diagnostic, Label, Severity},
    ir::{Grammar, Rule},
};

/// The module that parses a grammar.
#[derive(Debug)]
#[non_exhaustive]
pub struct Module {
    /// The items of the module.
    pub items: TokenStream,
    /// Each change to the grammar that would make its parser faster, as text
    /// that shows where in the grammar it is.
    pub suggestions: Vec<String>,
    /// The declaration of each type of the module, after the rule it comes
    /// from, as Rust writes it.
    pub types: String,
}

/// How to generate the module that parses a grammar.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Options {
    /// How many matches of recursive rules the parser follows inside one
    /// another before it stops with an error, instead of running out of stack.
    /// `None` keeps the runtime's default.
    pub depth: Option<usize>,
}

/// Returns the module that parses the grammar `source`, or every problem that
/// stops it, rendered with `path` naming the grammar's file.
///
/// Warnings stop it too unless the grammar expects them, so a generated parser
/// always does what its grammar means, or what the grammar says it intends.
/// Suggestions never stop it.
///
/// # Errors
///
/// Returns each problem as text that shows where in the grammar it is.
pub fn generate(
    source: &str,
    path: &str,
    options: Options,
) -> Result<Module, Vec<String>> {
    let render = |diagnostics: Vec<Diagnostic>| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(path, source, false))
            .collect()
    };
    let (grammar, diagnostics) = compile(source);
    let (suggestions, problems): (Vec<_>, Vec<_>) = diagnostics
        .into_iter()
        .partition(|diagnostic| diagnostic.severity == Severity::Suggestion);
    let Some(grammar) = grammar.filter(|_| problems.is_empty()) else {
        return Err(render(problems));
    };
    let shapes = shape::shapes(&grammar);
    let clashes = clashes(&grammar, &shapes);
    if !clashes.is_empty() {
        return Err(render(clashes));
    }
    let matched = rearrange::rearranged(&grammar);
    Ok(Module {
        items: code::module(
            &grammar,
            &matched,
            &shapes,
            source,
            options.depth,
        ),
        suggestions: render(suggestions),
        types: code::outline(&grammar, &shapes, source),
    })
}

/// What checking a grammar found.
#[derive(Debug)]
#[non_exhaustive]
pub struct Report {
    /// Every problem and suggestion, as text that starts with how serious it
    /// is and shows where in the grammar it is.
    pub findings: Vec<String>,
    /// Whether any of them would stop the build.
    pub stops_build: bool,
}

/// Returns every problem and suggestion about the grammar `source`, rendered
/// with `path` naming the grammar's file, without generating its module.
#[must_use]
pub fn check(source: &str, path: &str) -> Report {
    let (grammar, mut diagnostics) = compile(source);
    if let Some(grammar) = grammar {
        let clashes = clashes(&grammar, &shape::shapes(&grammar));
        // Types that would share a name already tell what a warning about
        // their names differing only in case would.
        diagnostics.retain(|diagnostic| {
            diagnostic.code != Some("case-only-names")
                || !clashes.iter().any(|clash| clash.span == diagnostic.span)
        });
        diagnostics.extend(clashes);
    }
    Report {
        findings: diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(path, source, true))
            .collect(),
        stops_build: diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity != Severity::Suggestion),
    }
}

/// Returns an error for every rule whose type would have the name of an
/// earlier rule's type, and for every conversion whose type one of them would
/// hide.
fn clashes(grammar: &Grammar, shapes: &[Option<Shape>]) -> Vec<Diagnostic> {
    let mut errors = same_names(grammar, shapes);
    errors.extend(hidden_conversions(grammar, shapes));
    errors
}

/// Returns an error for every rule whose type would have the name of an
/// earlier rule's type.
fn same_names(grammar: &Grammar, shapes: &[Option<Shape>]) -> Vec<Diagnostic> {
    let mut named = HashMap::new();
    let mut errors = Vec::new();
    for (rule, shape) in grammar.syntax_rules.iter().zip(shapes) {
        if matches!(shape, None | Some(Shape::Converted(_))) {
            continue;
        }
        let name = name::type_name(rule.meta_identifier.written());
        match named.entry(name.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(rule);
            }
            Entry::Occupied(entry) => errors.push(Diagnostic {
                severity: Severity::Error,
                code: None,
                message: format!(
                    "the types of `{}` and `{}` would both be named `{name}`",
                    entry.get().meta_identifier.spaced(),
                    rule.meta_identifier.spaced()
                ),
                span: rule.span,
                labels: vec![Label {
                    span: entry.get().span,
                    message: "this rule's type has the same name",
                }],
                help: Some("rename one of the rules".to_owned()),
                note: None,
            }),
        }
    }
    errors
}

/// Returns an error for every conversion whose type starts with the name of a
/// rule's type, since that type hides it in the generated module.
fn hidden_conversions(
    grammar: &Grammar,
    shapes: &[Option<Shape>],
) -> Vec<Diagnostic> {
    let types: HashMap<String, &Rule> = grammar
        .syntax_rules
        .iter()
        .zip(shapes)
        .filter(|(_, shape)| {
            matches!(shape, Some(Shape::Struct(_) | Shape::Enum(_)))
        })
        .map(|(rule, _)| {
            (
                name::type_name(rule.meta_identifier.written()).to_string(),
                rule,
            )
        })
        .collect();
    let mut errors = Vec::new();
    for rule in &grammar.syntax_rules {
        let Some(ty) = &rule.converts_to else {
            continue;
        };
        let first = ty.split("::").next().unwrap_or_default();
        let Some(hiding) = types.get(first) else {
            continue;
        };
        let path = if first == "String" {
            "::std::string::String".to_owned()
        } else {
            format!("super::{ty}")
        };
        errors.push(Diagnostic {
            severity: Severity::Error,
            code: None,
            message: format!(
                "`{first}` in `(* as {ty} *)` names the type of `{}`",
                hiding.meta_identifier.spaced()
            ),
            span: rule.span,
            labels: vec![Label {
                span: hiding.span,
                message: "this rule's type has the name",
            }],
            help: Some(format!(
                "write the path to the type, such as `{path}`, or rename the \
                 rule"
            )),
            note: None,
        });
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(source: &str) -> Vec<String> {
        generate(source, "g.ebnf", Options::default())
            .err()
            .unwrap_or_default()
    }

    #[test]
    fn generates_the_items_of_a_grammar_without_problems() {
        let items =
            generate("s = 'x', [a]; a = 'y';", "g.ebnf", Options::default())
                .unwrap()
                .items;
        let items = items.to_string();
        assert!(items.contains("pub struct S"), "{items}");
        assert!(items.contains("pub struct A"), "{items}");
        assert!(items.contains("pub fn parse"), "{items}");
    }

    #[test]
    fn tells_what_each_branch_can_start_with() {
        let items = generate(
            "s = a | b | 'w'; a = 'x', 'y'; b = ['v'], 'u';",
            "g.ebnf",
            Options::default(),
        )
        .unwrap()
        .items
        .to_string();
        assert!(items.contains("Some ('x'))"), "{items}");
        assert!(items.contains("Some ('u' ..= 'v'))"), "{items}");
        assert_eq!(items.matches("matches ! (parser").count(), 2, "{items}");
    }

    #[test]
    fn takes_the_longest_match() {
        let items = |source: String| {
            generate(&source, "g.ebnf", Options::default())
                .unwrap()
                .items
                .to_string()
        };
        let rules = "a = 'x'; b = 'x', 'y';";
        let longest = items(format!("s = a | b; {rules}"));
        assert!(longest.contains("longest"), "{longest}");
    }

    #[test]
    fn stops_at_the_one_branch_that_can_match_where_branches_start_apart() {
        let items = generate(
            "s = a | b; a = 'x'; b = 'y';",
            "g.ebnf",
            Options::default(),
        )
        .unwrap()
        .items
        .to_string();
        assert!(items.contains("found"), "{items}");
        assert!(!items.contains("longest"), "{items}");
    }

    #[test]
    fn generates_a_grammar_that_only_has_suggestions() {
        let source = "number = digit, {digit}; digit = ? U+0030..U+0039 ?;";
        let module = generate(source, "g.ebnf", Options::default()).unwrap();
        assert_eq!(module.suggestions.len(), 1);
        assert!(
            module.suggestions[0].starts_with(
                "suggestion[single-character-rule]: `digit` only ever"
            ),
            "{:?}",
            module.suggestions
        );
    }

    #[test]
    fn checks_a_grammar_with_how_serious_each_finding_is() {
        let report = check(
            "number = digit, {digit}; digit = ? U+0030..U+0039 ?;",
            "g.ebnf",
        );
        assert!(!report.stops_build);
        assert_eq!(report.findings.len(), 1);
        assert!(
            report.findings[0]
                .starts_with("suggestion[single-character-rule]: `digit`"),
            "{:?}",
            report.findings
        );
        let report = check("s = a;", "g.ebnf");
        assert!(report.stops_build);
        assert!(report.findings[0].starts_with("error: `a` is not defined"));
    }

    #[test]
    fn checks_names_that_differ_in_case_once_when_their_types_clash() {
        let report =
            check("s = digit, Digit; digit = '0'; Digit = '1';", "g.ebnf");
        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        assert!(report.findings[0].starts_with("error: the types of"));
    }

    #[test]
    fn leaves_a_branch_that_can_match_nothing_unguarded() {
        let items = generate(
            "s = (a | 'w'), 'z'; a = ['x'];",
            "g.ebnf",
            Options::default(),
        )
        .unwrap()
        .items
        .to_string();
        assert!(!items.contains("matches ! (parser"), "{items}");
    }

    #[test]
    fn stops_at_errors() {
        assert_eq!(
            problems("s = a;"),
            ["`a` is not defined\n \
              --> g.ebnf:1:5\n  \
              |\n\
              1 | s = a;\n  \
              |     ^\n  \
              |\n  \
              = help: define `a` with a syntax rule, or correct its spelling"]
        );
    }

    #[test]
    fn stops_at_warnings_the_grammar_does_not_expect() {
        assert_eq!(problems("(* greedy *) s = ['xy'], 'x';").len(), 1);
        assert_eq!(
            problems(
                "(* expect optional-follow-conflict: taken when it can be *) \
                 (* greedy *) s = ['xy'], 'x';"
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn stops_at_types_that_would_share_a_name() {
        let source = "s = digit, Digit; digit = '0'; \
                      (* expect case-only-names: as the standard writes it *) \
                      Digit = '1';";
        let found = problems(source);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].starts_with(
                "the types of `digit` and `Digit` would both be named \
                 `Digit`"
            ),
            "{found:?}"
        );
    }

    #[test]
    fn stops_at_a_conversion_that_a_rule_type_hides() {
        let found = problems(
            "pair = string, word; string = 'x'; (* as String *) word = 'y';",
        );
        assert_eq!(found.len(), 1);
        assert!(
            found[0].starts_with(
                "`String` in `(* as String *)` names the type of `string`"
            ),
            "{found:?}"
        );
        assert!(
            found[0].contains("such as `::std::string::String`"),
            "{found:?}"
        );
        assert_eq!(
            problems(
                "pair = string, word; string = 'x'; \
                 (* as ::std::string::String *) word = 'y';"
            ),
            Vec::<String>::new()
        );
    }
}
