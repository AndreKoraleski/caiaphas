//! Parse errors tell where the source of a grammar is not valid syntax, and
//! what the syntax needs there.

use std::{error::Error, fmt};

use crate::{
    annotation::Malformed,
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
    /// A comment reads as an annotation written wrongly.
    Malformed(Malformed),
    /// An annotation is not right before a syntax rule.
    AnnotationWithoutRule,
    /// A second `(* as T *)` comes before a rule that already converts.
    SecondConversion,
    /// `(* hide *)` and `(* inline *)` come before the same rule.
    HiddenInline,
}

/// What the syntax needs where a [`ParseError`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expected {
    MetaIdentifier,
    /// The same, where a `;` or `.` ends no rule.
    MetaIdentifierNotTerminator,
    /// The same, where `..` follows a rule as if it made a range.
    MetaIdentifierNotRange,
    DefiningSymbol,
    /// The same, where a `-` follows a name, as in `my-rule`.
    DefiningSymbolNotExcept,
    TerminatorSymbol,
    /// The same, where a second `-` follows an exception.
    TerminatorSymbolNotExcept,
    /// The same, where a closing bracket closes nothing.
    TerminatorSymbolNotClose,
    /// What can come between two items of a rule, or after its last.
    Separator,
    /// The same, where a `*` follows an item as in a regular expression.
    SeparatorNotRepetition,
    RepetitionSymbol,
    /// What a count repeats, where a second count follows the first.
    PrimaryNotCount,
    EndGroupSymbol,
    /// The same, where `/)` ends an optional sequence instead.
    EndGroupSymbolNotOption,
    EndOptionSymbol,
    EndRepeatSymbol,
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MetaIdentifier
            | Self::MetaIdentifierNotTerminator
            | Self::MetaIdentifierNotRange => "a rule name",
            Self::DefiningSymbol | Self::DefiningSymbolNotExcept => "`=`",
            Self::TerminatorSymbol
            | Self::TerminatorSymbolNotExcept
            | Self::TerminatorSymbolNotClose => "`;` or `.`",
            Self::Separator | Self::SeparatorNotRepetition => {
                "`,`, `|`, `;` or `.`"
            }
            Self::RepetitionSymbol => "`*` after the repetition count",
            Self::PrimaryNotCount => "what the count repeats",
            Self::EndGroupSymbol | Self::EndGroupSymbolNotOption => "`)`",
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
                f.write_str("the grammar has no rule")
            }
            ParseErrorKind::Malformed(malformed) => malformed.fmt(f),
            ParseErrorKind::AnnotationWithoutRule => f.write_str(
                "an `expect`, `hide`, `greedy`, `recover`, `inline` or `as` \
                 comment must come right before the syntax rule it is about",
            ),
            ParseErrorKind::SecondConversion => f.write_str(
                "a rule converts to one type, and another `(* as ... *)` \
                 already names one for this rule",
            ),
            ParseErrorKind::HiddenInline => f.write_str(
                "`(* hide *)` and `(* inline *)` cannot both mark a rule",
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
        let help = match &error.kind {
            ParseErrorKind::Lex(kind) => kind.help(),
            ParseErrorKind::Malformed(malformed) => Some(malformed.help()),
            ParseErrorKind::UnexpectedToken(Expected::Separator) => Some(
                "separate the items of a sequence with `,`, as in `'x', 'y'`"
                    .to_owned(),
            ),
            ParseErrorKind::UnexpectedToken(
                Expected::SeparatorNotRepetition,
            ) => Some(
                "a `*` only repeats a count, as in `3 * 'x'`; write `{'x'}` \
                 for any number of times, or `'x', {'x'}` for one or more"
                    .to_owned(),
            ),
            ParseErrorKind::UnexpectedToken(Expected::TerminatorSymbol) => {
                Some("end the rule with `;`".to_owned())
            }
            ParseErrorKind::UnexpectedToken(Expected::DefiningSymbol) => {
                Some("define a rule with `=`, as in `a = 'x';`".to_owned())
            }
            ParseErrorKind::UnexpectedToken(
                Expected::DefiningSymbolNotExcept,
            ) => Some(
                "write the words of a name apart, as `my rule`, since `-` \
                 writes an exception"
                    .to_owned(),
            ),
            ParseErrorKind::UnexpectedToken(
                Expected::TerminatorSymbolNotExcept,
            ) => Some(
                "a term takes one exception, so group what it takes away, as \
                 `'a' - ('b' | 'c')`"
                    .to_owned(),
            ),
            ParseErrorKind::UnexpectedToken(
                Expected::TerminatorSymbolNotClose,
            ) => {
                Some("remove it, since nothing before it opens".to_owned())
            }
            ParseErrorKind::UnexpectedToken(Expected::PrimaryNotCount) => Some(
                "a factor takes one count, so multiply them, as `6 * 'x'` \
                 does for `2 * 3 * 'x'`"
                    .to_owned(),
            ),
            ParseErrorKind::UnexpectedToken(
                Expected::EndGroupSymbolNotOption,
            ) => Some(
                "`/)` ends an optional sequence that `(/` starts; write `/ )` \
                 for a `/` between alternatives and then the `)`"
                    .to_owned(),
            ),
            ParseErrorKind::SecondConversion => Some(
                "keep the `(* as ... *)` that names the type it should have"
                    .to_owned(),
            ),
            ParseErrorKind::HiddenInline => Some(
                "keep one: `hide` leaves the rule out of the types, and \
                 `inline` gives what it holds to the rules that hold it"
                    .to_owned(),
            ),
            ParseErrorKind::UnexpectedToken(
                Expected::MetaIdentifierNotTerminator,
            ) => Some("remove it, since it ends no rule".to_owned()),
            ParseErrorKind::UnexpectedToken(
                Expected::MetaIdentifierNotRange,
            ) => Some(
                "a `.` ends a rule, as `;` does; write a range of \
                     characters as `? /[a-z]/ ?` or `? U+0061..U+007A ?`"
                    .to_owned(),
            ),
            _ => None,
        };
        Self {
            severity: Severity::Error,
            code: None,
            message,
            span: error.span,
            labels: Vec::new(),
            help,
            note: None,
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
                code: None,
                message: "expected `;` or `.`, found the end of the grammar"
                    .to_owned(),
                span: Span { start: 7, end: 7 },
                labels: Vec::new(),
                help: None,
                note: None,
            }
        );
    }

    #[test]
    fn keeps_what_caused_an_error_in_its_diagnostic() {
        let (_, errors) = parse("(* '' *) a = 'x';");
        assert_eq!(
            Diagnostic::from(errors[0]).message,
            "invalid comment: terminal string is empty"
        );
    }

    #[test]
    fn helps_with_what_other_notations_write() {
        let helps = |source| -> Vec<String> {
            parse(source)
                .1
                .into_iter()
                .filter_map(|error| Diagnostic::from(error).help)
                .collect()
        };
        assert_eq!(
            helps("a = 'x'..'z';"),
            [
                "a `.` ends a rule, as `;` does; write a range of characters \
                 as `? /[a-z]/ ?` or `? U+0061..U+007A ?`"
            ]
        );
        assert_eq!(
            helps("a = 'x';; b = 'y';"),
            ["remove it, since it ends no rule"]
        );
        assert!(
            helps("a -> 'x';").contains(
                &"define a rule with `=`, as in `a = 'x';`".to_owned()
            )
        );
        assert_eq!(
            helps("my-rule = 'x';"),
            ["write the words of a name apart, as `my rule`, since `-` \
                 writes an exception"]
        );
        assert_eq!(
            helps("a = 'a' - 'b' - 'c';"),
            [
                "a term takes one exception, so group what it takes away, as \
                 `'a' - ('b' | 'c')`"
            ]
        );
        assert_eq!(
            helps("a = 'a'];"),
            ["remove it, since nothing before it opens"]
        );
        assert_eq!(
            helps("a = 2 * 3 * 'x';"),
            ["a factor takes one count, so multiply them, as `6 * 'x'` \
                 does for `2 * 3 * 'x'`"]
        );
        assert_eq!(
            helps("a = ('a' /);"),
            [
                "`/)` ends an optional sequence that `(/` starts; write `/ )` \
                 for a `/` between alternatives and then the `)`"
            ]
        );
        assert_eq!(
            helps("(* as u8 *) (* as u16 *) a = '1'; b = a;"),
            ["keep the `(* as ... *)` that names the type it should have"]
        );
        assert_eq!(
            helps("(* hide *) (* inline *) a = 'x'; b = a;"),
            ["keep one: `hide` leaves the rule out of the types, and \
                 `inline` gives what it holds to the rules that hold it"]
        );
        assert_eq!(
            helps("a = 'x'; # a note"),
            [
                "write a comment as `(* a comment *)`, or `'#'` in quotes to \
                 match it"
            ]
        );
        assert_eq!(
            helps("a = 'x', b?; b = 'y';"),
            [
                "end it with `?`; for an optional `x`, write `[x]` instead of \
              `x?`"
            ]
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
                "the grammar has no rule",
            ),
            (
                error(ParseErrorKind::Malformed(Malformed::NoColon)),
                "an expectation needs a `:` between its code and its reason",
            ),
            (
                error(ParseErrorKind::Malformed(Malformed::Misspelled {
                    meant: "hide",
                })),
                "this comment reads like `(* hide *)` written wrongly",
            ),
            (
                error(ParseErrorKind::AnnotationWithoutRule),
                "an `expect`, `hide`, `greedy`, `recover`, `inline` or `as` \
                 comment must come right before the syntax rule it is about",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
        assert_eq!(
            parse("a = @;").1[0].to_string(),
            "unexpected character `@`"
        );
    }

    #[test]
    fn error_gives_the_source_of_a_lexer_error() {
        let (_, errors) = parse("(* '' *) a = 'x';");
        assert_eq!(
            errors[0].source().map(ToString::to_string),
            Some("terminal string is empty".to_owned())
        );
    }
}
