//! The parser turns the tokens of a grammar into its syntax tree.

mod definitions_list;
mod single_definition;
mod syntactic_factor;
mod syntactic_primary;
mod syntactic_term;
mod syntax;
mod syntax_rule;

use std::{error::Error, fmt};

pub(crate) use definitions_list::DefinitionsList;
pub(crate) use single_definition::SingleDefinition;
pub(crate) use syntactic_factor::SyntacticFactor;
pub(crate) use syntactic_primary::{SyntacticPrimary, SyntacticPrimaryKind};
pub(crate) use syntactic_term::SyntacticTerm;
pub(crate) use syntax::Syntax;
pub(crate) use syntax_rule::SyntaxRule;

use crate::{
    diagnostic::{Diagnostic, Severity},
    expectation::Comment,
    lexer::{CommentlessSymbol, LexErrorKind, Lexer, Symbol, Token},
    span::Span,
};

/// Returns the syntax of `source`, and every error found while reading it.
///
/// After an error in a syntax rule it skips to the next terminator symbol and
/// goes on, so the syntax holds every rule that could be read. Expectations
/// right before a rule that could not be read are left out with it.
pub(crate) fn parse(source: &str) -> (Syntax, Vec<ParseError>) {
    let mut parser = Parser::new(source);
    let mut rules = Vec::new();
    // Where each rule the parser tried to read is, even if it could not.
    let mut attempts = Vec::new();
    while let Some(start) = parser.peek().map(|token| token.span.start) {
        match SyntaxRule::parse(&mut parser) {
            Ok(rule) => {
                attempts.push(rule.span);
                rules.push(rule);
            }
            Err(error) => {
                parser.errors.push(error);
                parser.skip_past_terminator();
                attempts.push(Span {
                    start,
                    end: parser.previous_end,
                });
            }
        }
    }
    let mut errors = parser.errors;
    for &span in parser.lexer.comments() {
        let text = &source[span.range()];
        let expectation = match Comment::read(text, span) {
            Comment::Ordinary => continue,
            Comment::Expectation(expectation) => expectation,
            Comment::Malformed => {
                errors.push(ParseError {
                    kind: ParseErrorKind::MalformedExpectation,
                    span,
                });
                continue;
            }
        };
        let inside = attempts
            .iter()
            .any(|rule| rule.start < span.start && span.end < rule.end);
        match attempts.iter().find(|rule| rule.start >= span.end) {
            Some(next) if !inside => {
                if let Some(rule) =
                    rules.iter_mut().find(|rule| rule.span == *next)
                {
                    rule.expectations.push(expectation);
                }
            }
            _ => errors.push(ParseError {
                kind: ParseErrorKind::ExpectationWithoutRule,
                span,
            }),
        }
    }
    if rules.is_empty() && errors.is_empty() {
        errors.push(ParseError {
            kind: ParseErrorKind::NoSyntaxRule,
            span: Span {
                start: 0,
                end: source.len(),
            },
        });
    }
    errors.sort_by_key(|error| error.span.start);
    (
        Syntax {
            syntax_rules: rules,
        },
        errors,
    )
}

/// Reads tokens one at a time, with one token of lookahead.
///
/// Lexer errors are collected as they come, so the rules only see tokens.
struct Parser<'a> {
    lexer: Lexer<'a>,
    next: Option<Token>,
    /// Where the last token read ends.
    previous_end: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        let mut parser = Self {
            lexer: Lexer::new(source),
            next: None,
            previous_end: 0,
            errors: Vec::new(),
        };
        parser.next = parser.pull();
        parser
    }

    /// Returns the next token from the lexer, collecting its errors.
    fn pull(&mut self) -> Option<Token> {
        for token in self.lexer.by_ref() {
            match token {
                Ok(token) => return Some(token),
                Err(error) => self.errors.push(ParseError {
                    kind: ParseErrorKind::Lex(error.kind),
                    span: error.span,
                }),
            }
        }
        None
    }

    fn peek(&self) -> Option<&Token> {
        self.next.as_ref()
    }

    fn peek_symbol(&self) -> Option<Symbol> {
        match self.peek()?.symbol {
            CommentlessSymbol::Symbol(symbol) => Some(symbol),
            _ => None,
        }
    }

    /// Moves past the next token, and returns it.
    fn bump(&mut self) -> Option<Token> {
        let token = self.next.take()?;
        self.previous_end = token.span.end;
        self.next = self.pull();
        Some(token)
    }

    /// Moves past the next token if it is `symbol`, and returns its span.
    fn eat(&mut self, symbol: Symbol) -> Option<Span> {
        if self.peek_symbol() == Some(symbol) {
            self.bump().map(|token| token.span)
        } else {
            None
        }
    }

    /// Moves past the next token if it is `symbol`, and returns its span, or
    /// returns an error saying what was expected.
    fn expect(
        &mut self,
        symbol: Symbol,
        expected: Expected,
    ) -> Result<Span, ParseError> {
        self.eat(symbol).ok_or_else(|| self.unexpected(expected))
    }

    /// Returns an error saying `expected` was not the next token.
    fn unexpected(&self, expected: Expected) -> ParseError {
        match self.peek() {
            Some(token) => ParseError {
                kind: ParseErrorKind::Expected(expected),
                span: token.span,
            },
            None => ParseError {
                kind: ParseErrorKind::UnexpectedEnd(expected),
                span: self.empty_span(),
            },
        }
    }

    /// Returns an empty span where the last token read ends.
    fn empty_span(&self) -> Span {
        Span {
            start: self.previous_end,
            end: self.previous_end,
        }
    }

    /// Moves past every token up to and including the next terminator symbol.
    fn skip_past_terminator(&mut self) {
        while let Some(token) = self.bump() {
            if token.symbol == CommentlessSymbol::Symbol(Symbol::Terminator) {
                return;
            }
        }
    }
}

/// Error returned when part of the source of a grammar cannot be read as a
/// syntax rule.
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
    Expected(Expected),
    /// The source ends where the syntax needs more.
    UnexpectedEnd(Expected),
    /// The source has no syntax rule at all.
    NoSyntaxRule,
    /// A comment starts with `expect` but is not written as an expectation.
    MalformedExpectation,
    /// An expectation is not right before a syntax rule.
    ExpectationWithoutRule,
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
            ParseErrorKind::Expected(expected) => {
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
            ParseErrorKind::ExpectationWithoutRule => f.write_str(
                "an expectation must come right before the syntax rule it is \
                 about",
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
    /// Ends the message with every error that caused this one, since only
    /// the innermost says what is wrong, such as an empty terminal string.
    fn from(error: ParseError) -> Self {
        let mut message = error.to_string();
        let mut cause = error.source();
        while let Some(error) = cause {
            message = format!("{message}: {error}");
            cause = error.source();
        }
        Self {
            severity: Severity::Error,
            code: None,
            message,
            span: error.span,
            labels: Vec::new(),
            help: None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fmt::Write;

    use super::*;
    use crate::lexer::ParseCommentlessSymbolError;

    /// Writes `syntax` back as EBNF, in the canonical spelling.
    fn render(syntax: &Syntax) -> String {
        let mut text = String::new();
        for rule in &syntax.syntax_rules {
            write!(text, "{} = ", rule.meta_identifier.as_str()).unwrap();
            render_definitions(&mut text, &rule.definitions_list);
            text.push(';');
        }
        text
    }

    fn render_definitions(text: &mut String, list: &DefinitionsList) {
        for (index, definition) in list.single_definitions.iter().enumerate() {
            if index > 0 {
                text.push_str(" | ");
            }
            for (index, term) in definition.syntactic_terms.iter().enumerate()
            {
                if index > 0 {
                    text.push_str(", ");
                }
                render_factor(text, &term.syntactic_factor);
                if let Some(exception) = &term.syntactic_exception {
                    text.push_str(" - ");
                    render_factor(text, exception);
                }
            }
        }
    }

    fn render_factor(text: &mut String, factor: &SyntacticFactor) {
        if let Some(count) = factor.integer {
            write!(text, "{} * ", count.get()).unwrap();
        }
        render_primary(text, &factor.syntactic_primary);
    }

    fn render_primary(text: &mut String, primary: &SyntacticPrimary) {
        let (open, list, close) = match &primary.kind {
            SyntacticPrimaryKind::OptionalSequence(list) => ('[', list, ']'),
            SyntacticPrimaryKind::RepeatedSequence(list) => ('{', list, '}'),
            SyntacticPrimaryKind::GroupedSequence(list) => ('(', list, ')'),
            SyntacticPrimaryKind::MetaIdentifier(name) => {
                text.push_str(name.as_str());
                return;
            }
            SyntacticPrimaryKind::TerminalString(string) => {
                write!(text, "'{}'", string.as_str()).unwrap();
                return;
            }
            SyntacticPrimaryKind::SpecialSequence(sequence) => {
                let range = sequence.range();
                write!(
                    text,
                    "? U+{:04X}..U+{:04X} ?",
                    u32::from(*range.start()),
                    u32::from(*range.end())
                )
                .unwrap();
                return;
            }
            SyntacticPrimaryKind::EmptySequence => return,
        };
        text.push(open);
        render_definitions(text, list);
        text.push(close);
    }

    /// Returns the syntax of `source`, asserting it has no errors.
    pub(crate) fn parse_valid(source: &str) -> Syntax {
        let (syntax, errors) = parse(source);
        assert_eq!(errors, [], "{source:?}");
        syntax
    }

    fn kinds(source: &str) -> Vec<ParseErrorKind> {
        parse(source)
            .1
            .into_iter()
            .map(|error| error.kind)
            .collect()
    }

    #[test]
    fn parses_a_rule() {
        assert_eq!(
            render(&parse_valid("digit = '0' | '1';")),
            "digit = '0' | '1';"
        );
    }

    #[test]
    fn parses_every_spelling_of_the_symbols() {
        assert_eq!(
            render(&parse_valid("a = b / c ! d. e = (/ f /), (: g :);")),
            "a = b | c | d;e = [f], {g};"
        );
    }

    #[test]
    fn parses_terms_factors_and_exceptions() {
        assert_eq!(
            render(&parse_valid("a = b, 3 * c - d, e - 2 * f;")),
            "a = b, 3 * c - d, e - 2 * f;"
        );
    }

    #[test]
    fn parses_nested_brackets() {
        assert_eq!(
            render(&parse_valid("a = [b, {c | (d)}];")),
            "a = [b, {c | (d)}];"
        );
    }

    #[test]
    fn parses_special_sequences_and_terminal_strings() {
        assert_eq!(
            render(&parse_valid("a = ? U+0041 ?, \"b\";")),
            "a = ? U+0041..U+0041 ?, 'b';"
        );
    }

    #[test]
    fn parses_empty_sequences() {
        assert_eq!(
            render(&parse_valid("a = ; b = c | ; d = [ ], 3 * ;")),
            "a = ;b = c | ;d = [], 3 * ;"
        );
    }

    #[test]
    fn parses_several_rules_with_gaps_and_comments() {
        let source =
            "(* digits *)\nzero = '0';\n\none (* the next *) = '1';\n";
        assert_eq!(render(&parse_valid(source)), "zero = '0';one = '1';");
    }

    #[test]
    fn spans_cover_each_node() {
        let source = "a = b, 3 * [c];";
        let syntax = parse_valid(source);
        let rule = &syntax.syntax_rules[0];
        let text = |span: Span| &source[span.range()];
        assert_eq!(text(rule.span), "a = b, 3 * [c];");
        assert_eq!(text(rule.definitions_list.span), "b, 3 * [c]");
        let terms =
            &rule.definitions_list.single_definitions[0].syntactic_terms;
        assert_eq!(text(terms[0].span), "b");
        assert_eq!(text(terms[1].span), "3 * [c]");
        assert_eq!(
            text(terms[1].syntactic_factor.syntactic_primary.span),
            "[c]"
        );
    }

    #[test]
    fn empty_sequences_have_an_empty_span_after_the_last_token() {
        let syntax = parse_valid("a =  ;");
        let span = syntax.syntax_rules[0].definitions_list.span;
        assert_eq!(span, Span { start: 3, end: 3 });
    }

    #[test]
    fn rejects_a_rule_without_a_meta_identifier() {
        let (syntax, errors) = parse("= b; c = d;");
        assert_eq!(render(&syntax), "c = d;");
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::Expected(Expected::MetaIdentifier),
                span: Span { start: 0, end: 1 },
            }]
        );
    }

    #[test]
    fn rejects_a_rule_without_a_defining_symbol() {
        let (syntax, errors) = parse("a 'b'; c = d;");
        assert_eq!(render(&syntax), "c = d;");
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::Expected(Expected::DefiningSymbol),
                span: Span { start: 2, end: 5 },
            }]
        );
    }

    #[test]
    fn joins_meta_identifiers_across_gaps() {
        assert_eq!(
            render(&parse_valid("syntax rule = b c;")),
            "syntaxrule = bc;"
        );
    }

    #[test]
    fn rejects_terms_without_a_concatenate_symbol() {
        assert_eq!(
            kinds("a = b 'c';"),
            [ParseErrorKind::Expected(Expected::TerminatorSymbol)]
        );
    }

    #[test]
    fn rejects_a_repetition_count_without_a_repetition_symbol() {
        assert_eq!(
            kinds("a = 3 b;"),
            [ParseErrorKind::Expected(Expected::RepetitionSymbol)]
        );
    }

    #[test]
    fn rejects_unclosed_brackets() {
        for (source, expected) in [
            ("a = (b;", Expected::EndGroupSymbol),
            ("a = [b;", Expected::EndOptionSymbol),
            ("a = {b;", Expected::EndRepeatSymbol),
            ("a = (b];", Expected::EndGroupSymbol),
        ] {
            assert_eq!(
                kinds(source),
                [ParseErrorKind::Expected(expected)],
                "{source:?}"
            );
        }
    }

    #[test]
    fn rejects_a_grammar_that_ends_inside_a_rule() {
        let (_, errors) = parse("a = b");
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::UnexpectedEnd(
                    Expected::TerminatorSymbol
                ),
                span: Span { start: 5, end: 5 },
            }]
        );
    }

    #[test]
    fn goes_on_at_the_rule_after_an_error() {
        let (syntax, errors) = parse("a = (b; c = d; e f; g = h;");
        assert_eq!(render(&syntax), "c = d;g = h;");
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn collects_lexer_errors_and_reads_around_them() {
        let (syntax, errors) = parse("a = @ b;");
        assert_eq!(render(&syntax), "a = b;");
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::Lex(LexErrorKind::CommentlessSymbol(
                    ParseCommentlessSymbolError::NotCommentless('@')
                )),
                span: Span { start: 4, end: 5 },
            }]
        );
    }

    #[test]
    fn sorts_errors_by_where_they_are() {
        let starts: Vec<usize> = parse("a = b 'c'; @ d = 'e' f;")
            .1
            .iter()
            .map(|error| error.span.start)
            .collect();
        assert_eq!(starts, [6, 11, 21]);
    }

    /// Returns the codes of the expectations of each syntax rule.
    fn expectations(syntax: &Syntax) -> Vec<Vec<&str>> {
        syntax
            .syntax_rules
            .iter()
            .map(|rule| {
                rule.expectations
                    .iter()
                    .map(|expectation| expectation.code.as_str())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn attaches_expectations_to_the_rule_after_them() {
        let source = "(* expect empty-rule: kept for later *) a = ; \
                      (* a note *) b = 'x'; \
                      (* expect shadowed-branch: first *) \
                      (* expect unreachable-rule: second *) c = 'y';";
        let syntax = parse_valid(source);
        assert_eq!(
            expectations(&syntax),
            [
                vec!["empty-rule"],
                vec![],
                vec!["shadowed-branch", "unreachable-rule"]
            ]
        );
        let expectation = &syntax.syntax_rules[0].expectations[0];
        assert_eq!(expectation.reason, "kept for later");
        assert_eq!(expectation.span, Span { start: 0, end: 39 });
    }

    #[test]
    fn leaves_out_the_expectations_of_a_rule_it_could_not_read() {
        let (syntax, errors) =
            parse("(* expect empty-rule: kept *) a = (b; c = 'y';");
        assert_eq!(expectations(&syntax), [Vec::<&str>::new()]);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::Expected(Expected::EndGroupSymbol),
                span: Span { start: 36, end: 37 },
            }]
        );
    }

    #[test]
    fn rejects_a_malformed_expectation() {
        let (syntax, errors) = parse("(* expect empty-rule *) a = 'x';");
        assert_eq!(expectations(&syntax), [Vec::<&str>::new()]);
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::MalformedExpectation,
                span: Span { start: 0, end: 23 },
            }]
        );
    }

    #[test]
    fn rejects_an_expectation_without_a_rule_after_it() {
        for (source, start) in [
            ("a = 'x'; (* expect empty-rule: kept *)", 9),
            ("a = 'x', (* expect empty-rule: kept *) 'y';", 9),
            ("a = 'x', (* expect empty-rule: kept *) 'y'; b = 'z';", 9),
        ] {
            let (syntax, errors) = parse(source);
            assert!(
                syntax
                    .syntax_rules
                    .iter()
                    .all(|rule| rule.expectations.is_empty())
            );
            assert_eq!(
                errors,
                [ParseError {
                    kind: ParseErrorKind::ExpectationWithoutRule,
                    span: Span {
                        start,
                        end: start + 29,
                    },
                }],
                "{source:?}"
            );
        }
    }

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
    fn rejects_a_grammar_without_syntax_rules() {
        for source in ["", "  \n", "(* nothing here *)"] {
            assert_eq!(
                parse(source).1,
                [ParseError {
                    kind: ParseErrorKind::NoSyntaxRule,
                    span: Span {
                        start: 0,
                        end: source.len(),
                    },
                }],
                "{source:?}"
            );
        }
    }

    #[test]
    fn reports_no_missing_rules_when_rules_have_errors() {
        assert_eq!(
            kinds("@"),
            [ParseErrorKind::Lex(LexErrorKind::CommentlessSymbol(
                ParseCommentlessSymbolError::NotCommentless('@')
            ))]
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
                error(ParseErrorKind::Expected(Expected::DefiningSymbol)),
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
                error(ParseErrorKind::ExpectationWithoutRule),
                "an expectation must come right before the syntax rule it is \
                 about",
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
