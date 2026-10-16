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
/// diagnostic about it. Those that rearranging takes care of are left out when
/// the grammar is to be `rearranged`.
pub(crate) fn compile(
    source: &str,
    rearranged: bool,
) -> (Option<Grammar>, Vec<Diagnostic>) {
    let (syntax, errors) = parse(source);
    if !errors.is_empty() {
        return (None, diagnostics(errors));
    }
    let (grammar, errors) = build(&syntax);
    if !errors.is_empty() {
        return (None, diagnostics(errors));
    }
    let mut findings = analyse(&grammar);
    findings.retain(|finding| !(rearranged && finding.is_rearranged_away()));
    let diagnostics = diagnostics(findings);
    let grammar = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Error)
        .then_some(grammar);
    (grammar, diagnostics)
}

/// Returns each of `problems` as a diagnostic.
fn diagnostics<T: Into<Diagnostic>>(problems: Vec<T>) -> Vec<Diagnostic> {
    problems.into_iter().map(Into::into).collect()
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

    fn severities(diagnostics: &[Diagnostic]) -> Vec<Severity> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.severity)
            .collect()
    }

    #[test]
    fn returns_a_grammar_without_problems() {
        let (grammar, diagnostics) = compile("a = 'x', b; b = 'y';", false);
        assert_eq!(grammar.map(|grammar| grammar.syntax_rules.len()), Some(2));
        assert_eq!(diagnostics, []);
    }

    #[test]
    fn returns_a_grammar_with_its_warnings() {
        let (grammar, diagnostics) =
            compile("(* greedy *) a = ['xy'], 'x';", false);
        assert!(grammar.is_some());
        assert_eq!(severities(&diagnostics), [Severity::Warning]);
        assert_eq!(
            messages(&diagnostics),
            ["this optional sequence and what comes after it can both \
                 start with 'x', so a parser cannot tell whether to take it"]
        );
    }

    #[test]
    fn stops_after_parse_errors() {
        let (grammar, diagnostics) = compile("a = b", false);
        assert!(grammar.is_none());
        assert_eq!(
            messages(&diagnostics),
            ["expected `;` or `.`, found the end of the grammar"]
        );
    }

    #[test]
    fn stops_after_dictionary_errors() {
        let (grammar, diagnostics) = compile("a = a, 'x' | b;", false);
        assert!(grammar.is_none());
        assert_eq!(messages(&diagnostics), ["`b` is not defined"]);
    }

    #[test]
    fn keeps_no_grammar_with_an_error_in_the_analysis() {
        let (grammar, diagnostics) = compile("a = a, 'x' | 'y' | 'y';", false);
        assert!(grammar.is_none());
        assert_eq!(
            severities(&diagnostics),
            [Severity::Error, Severity::Warning]
        );
    }
}
