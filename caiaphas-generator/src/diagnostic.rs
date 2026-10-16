//! Diagnostics are what every stage reports about a grammar, in one shape, so
//! each can be shown with the lines of the grammar it points at.

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};

use crate::span::Span;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    /// The grammar cannot become a parser that works.
    Error,
    /// The parser works, but may not do what the grammar means.
    Warning,
    /// The parser works and does what the grammar means, but the grammar could
    /// be written so it is faster. A suggestion never stops the build.
    Suggestion,
}

/// A label points at another place a diagnostic involves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Label {
    pub(crate) span: Span,
    pub(crate) message: &'static str,
}

/// A diagnostic is one problem in a grammar, where it is, and how to fix it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub(crate) severity: Severity,
    /// The code of the finding, for a diagnostic from the analysis.
    pub(crate) code: Option<&'static str>,
    pub(crate) message: String,
    pub(crate) span: Span,
    pub(crate) labels: Vec<Label>,
    pub(crate) help: Option<String>,
    pub(crate) note: Option<String>,
}

impl Diagnostic {
    /// Returns the diagnostic as text, showing each line of `source` it points
    /// at, with `path` naming where the source comes from.
    ///
    /// The text of an error starts with how serious it is only when `named`.
    /// Otherwise whoever shows it adds that, as rustc does for a compile
    /// error.
    pub(crate) fn render(
        &self,
        path: &str,
        source: &str,
        named: bool,
    ) -> String {
        let level = match self.severity {
            Severity::Error => Level::ERROR,
            Severity::Warning => Level::WARNING,
            Severity::Suggestion => Level::INFO.with_name("suggestion"),
        };
        let title = if named || self.severity != Severity::Error {
            let title = level.primary_title(&self.message);
            match self.code {
                Some(code) => title.id(code),
                None => title,
            }
        } else {
            // A title without a name leaves out its code, so it goes in the
            // message instead.
            let message = match self.code {
                Some(code) => format!("[{code}] {}", self.message),
                None => self.message.clone(),
            };
            level.no_name().primary_title(message)
        };
        let snippet = Snippet::source(source)
            .path(path)
            .annotation(AnnotationKind::Primary.span(self.span.range()))
            .annotations(self.labels.iter().map(|label| {
                AnnotationKind::Context
                    .span(label.span.range())
                    .label(label.message)
            }));
        let mut group = title.element(snippet);
        if let Some(help) = &self.help {
            group = group.element(Level::HELP.message(help));
        }
        if let Some(note) = &self.note {
            group = group.element(Level::NOTE.message(note));
        }
        Renderer::plain().render(&[group])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code: None,
            message: "something is wrong".to_owned(),
            span: Span { start, end },
            labels: Vec::new(),
            help: None,
            note: None,
        }
    }

    #[test]
    fn shows_the_line_and_marks_the_span() {
        let mut found = diagnostic(9, 10);
        found.message = "`b` is not defined".to_owned();
        found.help = Some("define `b` with a syntax rule".to_owned());
        assert_eq!(
            found.render("grammar.ebnf", "a = 'x', b;\n", false),
            "`b` is not defined\n \
             --> grammar.ebnf:1:10\n  \
             |\n\
             1 | a = 'x', b;\n  \
             |          ^\n  \
             |\n  \
             = help: define `b` with a syntax rule"
        );
    }

    #[test]
    fn starts_with_how_serious_it_is_when_asked() {
        let source = "a = 'x';\n";
        assert!(
            diagnostic(4, 7)
                .render("g.ebnf", source, true)
                .starts_with("error: something is wrong")
        );
        let mut found = diagnostic(4, 7);
        found.severity = Severity::Suggestion;
        assert!(
            found
                .render("g.ebnf", source, true)
                .starts_with("suggestion: something is wrong")
        );
    }

    #[test]
    fn starts_with_the_code_of_a_finding() {
        let source = "a = 'x';\n";
        let mut found = diagnostic(4, 7);
        found.code = Some("left-recursion");
        assert!(
            found
                .render("g.ebnf", source, true)
                .starts_with("error[left-recursion]: something is wrong")
        );
        let unnamed = found.render("g.ebnf", source, false);
        assert!(
            unnamed.starts_with("[left-recursion] something is wrong"),
            "{unnamed}"
        );
        found.severity = Severity::Warning;
        assert!(
            found
                .render("g.ebnf", source, false)
                .starts_with("warning[left-recursion]: something is wrong")
        );
    }

    #[test]
    fn shows_labels_in_source_order() {
        let source = "a = 'x';\n\nb = 'y';\na = 'z';\n";
        let mut found = diagnostic(19, 27);
        found.labels = vec![
            Label {
                span: Span { start: 19, end: 20 },
                message: "this name",
            },
            Label {
                span: Span { start: 0, end: 8 },
                message: "first here",
            },
        ];
        assert_eq!(
            found.render("g.ebnf", source, false),
            "something is wrong\n \
             --> g.ebnf:4:1\n  \
             |\n\
             1 | a = 'x';\n  \
             | -------- first here\n\
             ...\n\
             4 | a = 'z';\n  \
             | -^^^^^^^\n  \
             | |\n  \
             | this name"
        );
    }

    #[test]
    fn marks_a_span_over_several_lines() {
        let source = "a = 'x'\r\n  | 'y';\r\n";
        assert_eq!(
            diagnostic(4, 16).render("g.ebnf", source, false),
            "something is wrong\n \
             --> g.ebnf:1:5\n  \
             |\n\
             1 |   a = 'x'\n  \
             |  _____^\n\
             2 | |   | 'y';\n  \
             | |_______^"
        );
    }
}
