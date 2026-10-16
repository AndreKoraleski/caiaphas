//! Generating turns a grammar into the Rust code of its parser and of the
//! types of its rules.

mod code;
mod matching;
mod name;
mod shape;

use std::collections::{HashMap, hash_map::Entry};

use proc_macro2::TokenStream;
use shape::Shape;

use crate::{
    compile::compile,
    diagnostic::{Diagnostic, Label, Severity},
    ir::Grammar,
};

/// The module that parses a grammar.
#[derive(Debug)]
pub struct Module {
    /// The items of the module.
    pub items: TokenStream,
    /// Each change to the grammar that would make its parser faster, as text
    /// that shows where in the grammar it is.
    pub suggestions: Vec<String>,
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
pub fn generate(source: &str, path: &str) -> Result<Module, Vec<String>> {
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
    Ok(Module {
        items: code::module(&grammar, &shapes, source),
        suggestions: render(suggestions),
    })
}

/// What checking a grammar found.
#[derive(Debug)]
pub struct Report {
    /// Every problem and suggestion, as text that starts with how serious it
    /// is and shows where in the grammar it is.
    pub findings: Vec<String>,
    /// Whether any of them would stop the build.
    pub stops_build: bool,
}

/// Returns every problem and suggestion about the grammar `source`, rendered
/// with `path` naming the grammar's file, without generating its module.
pub fn check(source: &str, path: &str) -> Report {
    let (grammar, mut diagnostics) = compile(source);
    if let Some(grammar) = grammar {
        diagnostics.extend(clashes(&grammar, &shape::shapes(&grammar)));
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
/// earlier rule's type.
fn clashes(grammar: &Grammar, shapes: &[Option<Shape>]) -> Vec<Diagnostic> {
    let mut named = HashMap::new();
    let mut errors = Vec::new();
    for (rule, shape) in grammar.syntax_rules.iter().zip(shapes) {
        if shape.is_none() {
            continue;
        }
        let name = name::type_name(rule.meta_identifier.written());
        match named.entry(name.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(rule);
            }
            Entry::Occupied(entry) => errors.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "the types of `{}` and `{}` would both be named `{name}`",
                    entry.get().meta_identifier.as_str(),
                    rule.meta_identifier.as_str()
                ),
                span: rule.span,
                labels: vec![Label {
                    span: entry.get().span,
                    message: "this rule's type has the same name",
                }],
                help: Some("rename one of the rules".to_owned()),
            }),
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(source: &str) -> Vec<String> {
        generate(source, "g.ebnf").err().unwrap_or_default()
    }

    #[test]
    fn generates_the_items_of_a_grammar_without_problems() {
        let items =
            generate("s = 'x', [a]; a = 'y';", "g.ebnf").unwrap().items;
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
        )
        .unwrap()
        .items
        .to_string();
        assert!(items.contains("Some ('x'))"), "{items}");
        assert!(items.contains("Some ('u' ..= 'v'))"), "{items}");
        assert_eq!(items.matches("matches ! (parser").count(), 2, "{items}");
    }

    #[test]
    fn takes_the_longest_match_unless_a_rule_asks_for_the_first() {
        let items = |source: String| {
            generate(&source, "g.ebnf").unwrap().items.to_string()
        };
        let rules = "a = 'x'; b = 'x', 'y';";
        let longest = items(format!("s = a | b; {rules}"));
        assert!(longest.contains("longest"), "{longest}");
        let first = items(format!("(* first match *) s = b | a; {rules}"));
        assert!(!first.contains("longest"), "{first}");
    }

    #[test]
    fn stops_at_the_one_branch_that_can_match_where_branches_start_apart() {
        let items = generate("s = a | b; a = 'x'; b = 'y';", "g.ebnf")
            .unwrap()
            .items
            .to_string();
        assert!(items.contains("found"), "{items}");
        assert!(!items.contains("longest"), "{items}");
    }

    #[test]
    fn generates_a_grammar_that_only_has_suggestions() {
        let source = "number = digit, {digit}; digit = ? U+0030..U+0039 ?;";
        let module = generate(source, "g.ebnf").unwrap();
        assert_eq!(module.suggestions.len(), 1);
        assert!(
            module.suggestions[0].starts_with("`digit` only ever matches"),
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
            report.findings[0].starts_with("suggestion: `digit`"),
            "{:?}",
            report.findings
        );
        let report = check("s = a;", "g.ebnf");
        assert!(report.stops_build);
        assert!(report.findings[0].starts_with("error: `a` is not defined"));
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
        assert_eq!(problems("s = ['xy'], 'x';").len(), 1);
        assert_eq!(
            problems(
                "(* expect optional-follow-conflict: taken when it can be *) \
                 s = ['xy'], 'x';"
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
}
