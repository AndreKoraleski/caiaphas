//! The parser turns the tokens of a grammar into its syntax tree.

mod annotations;
mod definitions_list;
mod error;
mod single_definition;
mod syntactic_factor;
mod syntactic_primary;
mod syntactic_term;
mod syntax;
mod syntax_rule;

pub(crate) use definitions_list::DefinitionsList;
pub(crate) use error::{Expected, ParseError, ParseErrorKind};
pub(crate) use single_definition::SingleDefinition;
pub(crate) use syntactic_factor::SyntacticFactor;
pub(crate) use syntactic_primary::{SyntacticPrimary, SyntacticPrimaryKind};
pub(crate) use syntactic_term::SyntacticTerm;
pub(crate) use syntax::Syntax;
pub(crate) use syntax_rule::SyntaxRule;

use crate::{
    lexer::{CommentlessSymbol, Lexer, Symbol, Token},
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
                // A token the lexer could not read leaves a gap in the rule,
                // so an error after it only repeats that one.
                let after_lex_error = parser.errors.iter().any(|earlier| {
                    matches!(earlier.kind, ParseErrorKind::Lex(_))
                        && earlier.span.start >= start
                });
                if !after_lex_error {
                    parser.errors.push(error);
                }
                parser.skip_past_terminator();
                attempts.push(Span {
                    start,
                    end: parser.previous_end,
                });
            }
        }
    }
    let mut errors = parser.errors;
    annotations::attach(
        source,
        parser.lexer.comments(),
        &attempts,
        &mut rules,
        &mut errors,
    );
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
                kind: ParseErrorKind::UnexpectedToken(expected),
                span: token.span,
            },
            None => ParseError {
                kind: ParseErrorKind::UnexpectedEnd(expected),
                span: self.empty_span(),
            },
        }
    }

    /// Returns the error for a rule that starts with something other than its
    /// name. A `;` or `.` there ends no rule, and a `.` right after the one
    /// that ended a rule reads as `..`, as if it made a range, so the rest of
    /// that is moved past too.
    fn no_rule_name(&mut self) -> ParseError {
        if self.peek_symbol() != Some(Symbol::Terminator) {
            return self.unexpected(Expected::MetaIdentifier);
        }
        let Some(span) = self.peek().map(|token| token.span) else {
            return self.unexpected(Expected::MetaIdentifier);
        };
        let source = self.lexer.source();
        let range = span.start > 0
            && span.start == self.previous_end
            && source[span.range()] == *"."
            && source[..span.start].ends_with('.');
        if !range {
            return self.unexpected(Expected::MetaIdentifierNotTerminator);
        }
        let error = self.unexpected(Expected::MetaIdentifierNotRange);
        self.bump();
        error
    }

    /// Returns the error for a rule that goes on where it should end. Before
    /// the name of the next rule, the rule lacks its `;`, so the error is
    /// where the rule ends. Before anything else a rule holds, it lacks a `,`.
    fn unfinished(&self) -> ParseError {
        let Some(token) = self.peek() else {
            return self.unexpected(Expected::TerminatorSymbol);
        };
        let after = self.lexer.source()[token.span.end..]
            .trim_start_matches(|c: char| c.is_ascii_whitespace());
        let next_rule =
            matches!(token.symbol, CommentlessSymbol::MetaIdentifier(_))
                && after.starts_with('=');
        let starts_item = matches!(
            token.symbol,
            CommentlessSymbol::MetaIdentifier(_)
                | CommentlessSymbol::Integer(_)
                | CommentlessSymbol::TerminalString(_)
                | CommentlessSymbol::SpecialSequence(_)
                | CommentlessSymbol::Symbol(
                    Symbol::StartGroup
                        | Symbol::StartOption
                        | Symbol::StartRepeat
                )
        );
        if next_rule {
            ParseError {
                kind: ParseErrorKind::UnexpectedToken(
                    Expected::TerminatorSymbol,
                ),
                span: self.empty_span(),
            }
        } else if starts_item {
            self.unexpected(Expected::Separator)
        } else if token.symbol == CommentlessSymbol::Symbol(Symbol::Repetition)
        {
            self.unexpected(Expected::SeparatorNotRepetition)
        } else {
            self.unexpected(Expected::TerminatorSymbol)
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

#[cfg(test)]
pub(crate) mod tests {
    use std::fmt::Write;

    use super::*;
    use crate::lexer::{LexErrorKind, ParseCommentlessSymbolError};

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
                write!(text, "? {:?} ?", sequence.pattern()).unwrap();
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
            "a = ? Characters(['A'..='A']) ?, 'b';"
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
                kind: ParseErrorKind::UnexpectedToken(
                    Expected::MetaIdentifier
                ),
                span: Span { start: 0, end: 1 },
            }]
        );
    }

    #[test]
    fn tells_a_terminator_that_ends_no_rule() {
        let (syntax, errors) = parse("a = 'x';; b = 'y';");
        assert_eq!(render(&syntax), "a = 'x';b = 'y';");
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::UnexpectedToken(
                    Expected::MetaIdentifierNotTerminator
                ),
                span: Span { start: 8, end: 9 },
            }]
        );
    }

    #[test]
    fn tells_two_dots_after_a_rule_once() {
        let (syntax, errors) = parse("a = 'x'..'z'; b = 'y';");
        assert_eq!(render(&syntax), "a = 'x';b = 'y';");
        assert_eq!(
            errors,
            [ParseError {
                kind: ParseErrorKind::UnexpectedToken(
                    Expected::MetaIdentifierNotRange
                ),
                span: Span { start: 8, end: 9 },
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
                kind: ParseErrorKind::UnexpectedToken(
                    Expected::DefiningSymbol
                ),
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
            [ParseErrorKind::UnexpectedToken(Expected::Separator)]
        );
        assert_eq!(
            kinds("a = 'x'*;"),
            [ParseErrorKind::UnexpectedToken(
                Expected::SeparatorNotRepetition
            )]
        );
    }

    #[test]
    fn ends_a_rule_that_lacks_its_terminator_where_it_ends() {
        let (_, errors) = parse("a = 'x'\nb = 'y';");
        assert_eq!(
            errors[0].kind,
            ParseErrorKind::UnexpectedToken(Expected::TerminatorSymbol)
        );
        assert_eq!(errors[0].span, Span { start: 7, end: 7 });
    }

    #[test]
    fn rejects_a_repetition_count_without_a_repetition_symbol() {
        assert_eq!(
            kinds("a = 3 b;"),
            [ParseErrorKind::UnexpectedToken(Expected::RepetitionSymbol)]
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
                [ParseErrorKind::UnexpectedToken(expected)],
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
    fn leaves_out_no_syntax_rule_when_another_error_explains_it() {
        assert_eq!(
            kinds("@"),
            [ParseErrorKind::Lex(LexErrorKind::CommentlessSymbol(
                ParseCommentlessSymbolError::NotCommentless('@')
            ))]
        );
    }

    #[test]
    fn leaves_out_what_a_token_it_could_not_read_leads_to() {
        let found = kinds("a = \"x;\nb = 'y';\n");
        assert_eq!(found.len(), 1);
        assert!(matches!(found[0], ParseErrorKind::Lex(_)), "{found:?}");
    }
}
