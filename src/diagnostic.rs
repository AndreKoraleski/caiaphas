//! Diagnostics are what every stage reports about a grammar, in one shape, so
//! each can be shown with the lines of the grammar it points at.

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};

use crate::span::Span;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Severity {
    /// The grammar cannot become a parser that works.
    Error,
    /// The parser works, but may not do what the grammar means.
    Warning,
}

/// A label points at another place a diagnostic involves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Label {
    pub(crate) span: Span,
    pub(crate) message: &'static str,
}

/// A diagnostic is one problem in a grammar, where it is, and how to fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub(crate) severity: Severity,
    /// The code an expectation names the problem by, if it has one.
    pub(crate) code: Option<&'static str>,
    pub(crate) message: String,
    pub(crate) span: Span,
    pub(crate) labels: Vec<Label>,
    pub(crate) help: Option<String>,
}

impl Diagnostic {
    /// Returns the diagnostic as text, showing each line of `source` it points
    /// at, with `path` naming where the source comes from.
    pub(crate) fn render(&self, path: &str, source: &str) -> String {
        let level = match self.severity {
            Severity::Error => Level::ERROR,
            Severity::Warning => Level::WARNING,
        };
        let mut title = level.primary_title(&self.message);
        if let Some(code) = self.code {
            title = title.id(code);
        }
        let snippet = Snippet::source(source)
            .path(path)
            .annotation(
                AnnotationKind::Primary.span(self.span.start..self.span.end),
            )
            .annotations(self.labels.iter().map(|label| {
                AnnotationKind::Context
                    .span(label.span.start..label.span.end)
                    .label(label.message)
            }));
        let mut group = title.element(snippet);
        if let Some(help) = &self.help {
            group = group.element(Level::HELP.message(help));
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
        }
    }

    #[test]
    fn shows_the_line_and_marks_the_span() {
        let mut found = diagnostic(9, 10);
        found.message = "`b` is not defined".to_owned();
        found.help = Some("define `b` with a syntax rule".to_owned());
        assert_eq!(
            found.render("grammar.ebnf", "a = 'x', b;\n"),
            "error: `b` is not defined\n \
             --> grammar.ebnf:1:10\n  \
             |\n\
             1 | a = 'x', b;\n  \
             |          ^\n  \
             |\n  \
             = help: define `b` with a syntax rule"
        );
    }

    #[test]
    fn names_the_code_and_shows_labels() {
        let source = "a = 'x';\n\nb = 'y';\na = 'z';\n";
        let mut found = diagnostic(19, 27);
        found.severity = Severity::Warning;
        found.code = Some("some-code");
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
            found.render("g.ebnf", source),
            "warning[some-code]: something is wrong\n \
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
            diagnostic(4, 16).render("g.ebnf", source),
            "error: something is wrong\n \
             --> g.ebnf:1:5\n  \
             |\n\
             1 |   a = 'x'\n  \
             |  _____^\n\
             2 | |   | 'y';\n  \
             | |_______^"
        );
    }
}
