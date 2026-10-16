//! Parse errors tell where the source of a grammar is not valid syntax, and
//! what the syntax needs there.

use std::{error::Error, fmt};

use crate::{
    diagnostic::{Diagnostic, Severity},
    lexer::LexErrorKind,
    span::Span,
};

/// Error returned when part of the source of a grammar is not valid syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParseError {
    pub(crate) kind: ParseErrorKind,
    /// What could not be read, or an empty span where something is missing.
    pub(crate) span: Span,
}

/// What went wrong in a [`ParseError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseErrorKind {
    /// The lexer could not read a token.
    Lex(LexErrorKind),
    /// The next token is not what the syntax needs.
    UnexpectedToken(Expected),
    /// The source ends where the syntax needs more.
    UnexpectedEnd(Expected),
    /// The source has no syntax rule at all.
    NoSyntaxRule,
    /// A comment starts with `expect` but is not written as an expectation.
    MalformedExpectation,
    /// A comment starts with `hide` but holds more than that.
    MalformedHide,
    /// A comment starts with `first match` but holds more than that.
    MalformedFirstMatch,
    /// A comment starts with `greedy` but holds more than that.
    MalformedGreedy,
    /// An annotation is not right before a syntax rule.
    AnnotationWithoutRule,
}

/// What the syntax needs where a [`ParseError`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expected {
    MetaIdentifier,
    DefiningSymbol,
    TerminatorSymbol,
    RepetitionSymbol,
    EndGroupSymbol,
    EndOptionSymbol,
    EndRepeatSymbol,
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MetaIdentifier => "a meta identifier",
            Self::DefiningSymbol => "`=`",
            Self::TerminatorSymbol => "`;` or `.`",
            Self::RepetitionSymbol => "`*` after the repetition count",
            Self::EndGroupSymbol => "`)`",
            Self::EndOptionSymbol => "`]` or `/)`",
            Self::EndRepeatSymbol => "`}` or `:)`",
        })
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParseErrorKind::Lex(kind) => kind.fmt(f),
            ParseErrorKind::UnexpectedToken(expected) => {
                write!(f, "expected {expected}")
            }
            ParseErrorKind::UnexpectedEnd(expected) => {
                write!(f, "expected {expected}, found the end of the grammar")
            }
            ParseErrorKind::NoSyntaxRule => {
                f.write_str("EBNF grammar has no syntax rule")
            }
            ParseErrorKind::MalformedExpectation => f.write_str(
                "an expectation is written `(* expect code-in-kebab-case: \
                 reason *)`",
            ),
            ParseErrorKind::MalformedHide => {
                f.write_str("a rule is hidden with `(* hide *)` alone")
            }
            ParseErrorKind::MalformedFirstMatch => f.write_str(
                "a rule takes the first match with `(* first match *)` alone",
            ),
            ParseErrorKind::MalformedGreedy => {
                f.write_str("a rule is made greedy with `(* greedy *)` alone")
            }
            ParseErrorKind::AnnotationWithoutRule => f.write_str(
                "an `expect`, `hide`, `first match` or `greedy` comment must \
                 come right before the syntax rule it is about",
            ),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ParseErrorKind::Lex(kind) => kind.source(),
            _ => None,
        }
    }
}

impl From<ParseError> for Diagnostic {
    /// Ends the message with every error that caused this one, since only the
    /// innermost says what is wrong, such as an empty terminal string.
    fn from(error: ParseError) -> Self {
        let mut message = error.to_string();
        let mut cause = error.source();
        while let Some(error) = cause {
            message = format!("{message}: {error}");
            cause = error.source();
        }
        Self {
            severity: Severity::Error,
            message,
            span: error.span,
            labels: Vec::new(),
            help: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn turns_an_error_into_a_diagnostic() {
        let (_, errors) = parse("a = 'x'");
        assert_eq!(
            Diagnostic::from(errors[0]),
            Diagnostic {
                severity: Severity::Error,
                message: "expected `;` or `.`, found the end of the grammar"
                    .to_owned(),
                span: Span { start: 7, end: 7 },
                labels: Vec::new(),
                help: None,
            }
        );
    }

    #[test]
    fn keeps_what_caused_an_error_in_its_diagnostic() {
        let (_, errors) = parse("a = '';");
        assert_eq!(
            Diagnostic::from(errors[0]).message,
            "invalid EBNF terminal string: EBNF terminal string is empty"
        );
    }

    #[test]
    fn error_displays_message() {
        let error = |kind| ParseError {
            kind,
            span: Span { start: 0, end: 0 },
        };
        let cases = [
            (
                error(ParseErrorKind::UnexpectedToken(
                    Expected::DefiningSymbol,
                )),
                "expected `=`",
            ),
            (
                error(ParseErrorKind::UnexpectedEnd(Expected::EndGroupSymbol)),
                "expected `)`, found the end of the grammar",
            ),
            (
                error(ParseErrorKind::NoSyntaxRule),
                "EBNF grammar has no syntax rule",
            ),
            (
                error(ParseErrorKind::MalformedExpectation),
                "an expectation is written `(* expect code-in-kebab-case: \
                 reason *)`",
            ),
            (
                error(ParseErrorKind::MalformedHide),
                "a rule is hidden with `(* hide *)` alone",
            ),
            (
                error(ParseErrorKind::MalformedFirstMatch),
                "a rule takes the first match with `(* first match *)` alone",
            ),
            (
                error(ParseErrorKind::MalformedGreedy),
                "a rule is made greedy with `(* greedy *)` alone",
            ),
            (
                error(ParseErrorKind::AnnotationWithoutRule),
                "an `expect`, `hide`, `first match` or `greedy` comment must \
                 come right before the syntax rule it is about",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
        assert_eq!(
            parse("a = @;").1[0].to_string(),
            "'@' does not start an EBNF commentless symbol"
        );
    }

    #[test]
    fn error_gives_the_source_of_a_lexer_error() {
        let (_, errors) = parse("a = '';");
        assert_eq!(
            errors[0].source().map(ToString::to_string),
            Some("EBNF terminal string is empty".to_owned())
        );
    }
}
