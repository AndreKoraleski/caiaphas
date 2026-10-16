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

/// Returns the items of the module that parses the grammar `source`, or every
/// problem that stops it, rendered with `path` naming the grammar's file.
///
/// Warnings stop it too unless the grammar expects them, so a generated parser
/// always does what its grammar means, or what the grammar says it intends.
/// Suggestions never stop it.
///
/// # Errors
///
/// Returns each problem as text that shows where in the grammar it is.
pub fn generate(source: &str, path: &str) -> Result<TokenStream, Vec<String>> {
    let render = |diagnostics: Vec<Diagnostic>| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(path, source))
            .collect()
    };
    let (grammar, mut diagnostics) = compile(source);
    diagnostics
        .retain(|diagnostic| diagnostic.severity != Severity::Suggestion);
    let Some(grammar) = grammar.filter(|_| diagnostics.is_empty()) else {
        return Err(render(diagnostics));
    };
    let shapes = shape::shapes(&grammar);
    let clashes = clashes(&grammar, &shapes);
    if !clashes.is_empty() {
        return Err(render(clashes));
    }
    Ok(code::module(&grammar, &shapes, source))
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
        let items = generate("s = 'x', [a]; a = 'y';", "g.ebnf").unwrap();
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
        .to_string();
        assert!(items.contains("Some ('x'))"), "{items}");
        assert!(items.contains("Some ('u' ..= 'v'))"), "{items}");
        assert_eq!(items.matches("matches ! (parser").count(), 2, "{items}");
    }

    #[test]
    fn takes_the_longest_match_unless_a_rule_asks_for_the_first() {
        let items =
            |source: String| generate(&source, "g.ebnf").unwrap().to_string();
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
            .to_string();
        assert!(items.contains("found"), "{items}");
        assert!(!items.contains("longest"), "{items}");
    }

    #[test]
    fn generates_a_grammar_that_only_has_suggestions() {
        let source = "number = digit, {digit}; digit = ? U+0030..U+0039 ?;";
        assert!(generate(source, "g.ebnf").is_ok());
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
