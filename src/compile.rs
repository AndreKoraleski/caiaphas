//! Compiling runs every stage on the source of a grammar. It stops after the
//! first stage that finds an error, since each stage needs what the one before
//! it built.

use crate::{
    analysis::analyse,
    diagnostic::{Diagnostic, Severity},
    dictionary::build,
    ir::Grammar,
    parser::parse,
};

/// Returns the grammar `source` defines, unless it has an error, and every
/// diagnostic about it.
pub(crate) fn compile(source: &str) -> (Option<Grammar>, Vec<Diagnostic>) {
    let (syntax, errors) = parse(source);
    if !errors.is_empty() {
        return (None, errors.into_iter().map(Diagnostic::from).collect());
    }
    let (grammar, errors) = build(&syntax);
    if !errors.is_empty() {
        return (None, errors.into_iter().map(Diagnostic::from).collect());
    }
    let diagnostics: Vec<Diagnostic> = analyse(&grammar)
        .into_iter()
        .map(Diagnostic::from)
        .collect();
    let grammar = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity == Severity::Warning)
        .then_some(grammar);
    (grammar, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect()
    }

    #[test]
    fn returns_a_grammar_without_problems() {
        let (grammar, diagnostics) = compile("a = 'x', b; b = 'y';");
        assert_eq!(grammar.map(|grammar| grammar.syntax_rules.len()), Some(2));
        assert_eq!(diagnostics, []);
    }

    #[test]
    fn returns_a_grammar_with_its_warnings() {
        let (grammar, diagnostics) = compile("a = ['xy'], 'x';");
        assert!(grammar.is_some());
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.severity, diagnostic.code))
                .collect::<Vec<_>>(),
            [(Severity::Warning, Some("optional-follow-conflict"))]
        );
    }

    #[test]
    fn stops_after_parse_errors() {
        let (grammar, diagnostics) = compile("a = b");
        assert!(grammar.is_none());
        assert_eq!(
            messages(&diagnostics),
            ["expected `;` or `.`, found the end of the grammar"]
        );
    }

    #[test]
    fn stops_after_dictionary_errors() {
        let (grammar, diagnostics) = compile("a = a, 'x' | b;");
        assert!(grammar.is_none());
        assert_eq!(messages(&diagnostics), ["`b` is not defined"]);
    }

    #[test]
    fn keeps_no_grammar_with_an_error_in_the_analysis() {
        let (grammar, diagnostics) = compile("a = a, 'x' | 'y';");
        assert!(grammar.is_none());
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.severity, diagnostic.code))
                .collect::<Vec<_>>(),
            [
                (Severity::Error, Some("left-recursion")),
                (Severity::Warning, Some("shadowed-branch")),
            ]
        );
    }
}
