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
/// the grammar is to be `rearranged`, and suggestions are left out with an
/// error, since no parser can be made faster before it works.
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
    let mut diagnostics = diagnostics(findings);
    let works = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Error);
    if !works {
        diagnostics
            .retain(|diagnostic| diagnostic.severity != Severity::Suggestion);
    }
    (works.then_some(grammar), diagnostics)
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
    fn tells_how_to_write_a_character_no_terminal_string_holds() {
        let help = |source: &str| compile(source, false).1.remove(0).help;
        assert_eq!(
            help("a = 'x\u{2013}y';").as_deref(),
            Some(
                "write `-` if that was meant, or write the character itself \
                 as `? U+2013 ?`"
            )
        );
        assert_eq!(
            help("a = 'caf\u{00E9}';").as_deref(),
            Some(
                "write it as `? U+00E9 ?` outside the quotes, since a \
                 special sequence can hold any character"
            )
        );
    }

    #[test]
    fn stops_after_dictionary_errors() {
        let (grammar, diagnostics) = compile("a = a, 'x' | b;", false);
        assert!(grammar.is_none());
        assert_eq!(messages(&diagnostics), ["`b` is not defined"]);
    }

    #[test]
    fn keeps_no_grammar_nor_suggestions_with_an_error_in_the_analysis() {
        let (grammar, diagnostics) = compile("a = a, 'x' | 'y' | 'y';", false);
        assert!(grammar.is_none());
        assert_eq!(
            severities(&diagnostics),
            [Severity::Error, Severity::Warning]
        );
    }
}
