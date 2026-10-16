//! The lexer turns the source of a grammar into tokens.

mod bracketed_textual_comment;
mod commentless_symbol;
mod gap_separator;
mod terminal_character;
mod token;

use std::{error::Error, fmt};

use bracketed_textual_comment::ParseBracketedTextualCommentError;
pub(crate) use commentless_symbol::ParseCommentlessSymbolError;
pub(crate) use commentless_symbol::{
    CommentlessSymbol, Integer, MetaIdentifier, SpecialSequence,
    TerminalString,
};
pub(crate) use terminal_character::Symbol;
pub(crate) use token::Token;

use crate::span::Span;

/// Reads the tokens of a grammar one at a time, skipping gaps and comments.
///
/// After an error it skips what could not be read and goes on, so one pass
/// finds every error that does not hide the ones after it.
#[derive(Debug, Clone)]
pub(crate) struct Lexer<'a> {
    source: &'a str,
    position: usize,
    /// Where each comment read so far is.
    comments: Vec<Span>,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            comments: Vec::new(),
        }
    }

    /// Returns where each comment read so far is, in order. A comment that
    /// could not be read is not one of them.
    pub(crate) fn comments(&self) -> &[Span] {
        &self.comments
    }

    /// Moves past the gaps and comments at the current position, or past the
    /// first comment that cannot be read.
    fn skip_gaps_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            self.position += gap_separator::length(self.rest());
            if !self.rest().starts_with("(*") {
                return Ok(());
            }
            match bracketed_textual_comment::length(self.rest()) {
                Ok(length) => {
                    let span = self.advance(length);
                    self.comments.push(span);
                }
                Err(error) => {
                    let length = unchecked_comment_length(self.rest());
                    return Err(self.skip(
                        LexErrorKind::BracketedTextualComment(error),
                        length,
                    ));
                }
            }
        }
    }

    /// Reads the token at the current position, or moves past what cannot be
    /// read.
    fn read_token(&mut self) -> Result<Token, LexError> {
        match CommentlessSymbol::read(self.rest()) {
            Ok((symbol, length)) => {
                let span = self.advance(length);
                Ok(Token { symbol, span })
            }
            Err(error) => {
                let length = recovery_length(error, self.rest());
                Err(self.skip(LexErrorKind::CommentlessSymbol(error), length))
            }
        }
    }

    fn rest(&self) -> &'a str {
        &self.source[self.position..]
    }

    /// Moves `length` bytes ahead, and returns the span moved over.
    fn advance(&mut self, length: usize) -> Span {
        let start = self.position;
        self.position += length;
        Span {
            start,
            end: self.position,
        }
    }

    /// Moves `length` bytes ahead, and returns an error over what it moved
    /// over.
    fn skip(&mut self, kind: LexErrorKind, length: usize) -> LexError {
        LexError {
            kind,
            span: self.advance(length),
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Result<Token, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(error) = self.skip_gaps_and_comments() {
            return Some(Err(error));
        }
        if self.rest().is_empty() {
            return None;
        }
        Some(self.read_token())
    }
}

/// Returns how much of `input` to skip after `error`, so that reading goes on
/// where the next token most likely starts.
///
/// Skips at least one character, so the lexer always moves.
fn recovery_length(error: ParseCommentlessSymbolError, input: &str) -> usize {
    let first = input.chars().next().map_or(0, char::len_utf8);
    let length = match error {
        // Only the value of the integer is wrong, so every digit is skipped.
        ParseCommentlessSymbolError::Integer(_) => {
            gap_separator::remove(input, |c| c.is_ascii_digit()).1
        }
        // A terminal string cannot go past the end of its line.
        ParseCommentlessSymbolError::TerminalString(_) => {
            let line = input.find('\n').map_or(input, |end| &input[..end]);
            line[first..]
                .find(&input[..first])
                .map_or(line.len(), |end| first + end + first)
        }
        ParseCommentlessSymbolError::SpecialSequence(_) => input[first..]
            .find('?')
            .map_or(input.len(), |end| first + end + 1),
        ParseCommentlessSymbolError::UnmatchedEndCommentSymbol => 2,
        ParseCommentlessSymbolError::EndOfInput
        | ParseCommentlessSymbolError::InvalidCharacter(_)
        | ParseCommentlessSymbolError::NotCommentless(_) => first,
    };
    length.max(first)
}

/// Returns how long the comment at the start of `input` is, counting only `(*`
/// and `*)`, or the length of `input` if it does not end.
///
/// Used to move past a comment that cannot be read as the standard says.
fn unchecked_comment_length(input: &str) -> usize {
    let mut depth = 0;
    let mut length = 0;
    while let Some(c) = input[length..].chars().next() {
        let after = &input[length..];
        if after.starts_with("(*") {
            depth += 1;
            length += 2;
        } else if after.starts_with("*)") {
            depth -= 1;
            length += 2;
            if depth == 0 {
                return length;
            }
        } else {
            length += c.len_utf8();
        }
    }
    length
}

/// Error returned when part of the source of a grammar cannot be read as a
/// token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LexError {
    pub(crate) kind: LexErrorKind,
    /// What the lexer skipped to go on reading.
    pub(crate) span: Span,
}

/// What went wrong in a [`LexError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LexErrorKind {
    CommentlessSymbol(ParseCommentlessSymbolError),
    BracketedTextualComment(ParseBracketedTextualCommentError),
}

impl LexErrorKind {
    /// Returns the error that caused this one, if any.
    pub(crate) fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CommentlessSymbol(error) => error.source(),
            Self::BracketedTextualComment(error) => error.source(),
        }
    }
}

impl fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommentlessSymbol(error) => error.fmt(f),
            Self::BracketedTextualComment(error) => error.fmt(f),
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl Error for LexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.kind.source()
    }
}

#[cfg(test)]
mod tests {
    use super::commentless_symbol::terminal_string::ParseTerminalStringError;
    use super::*;

    fn spans(source: &str) -> Vec<(usize, usize)> {
        Lexer::new(source)
            .map(|token| {
                let span = token.unwrap().span;
                (span.start, span.end)
            })
            .collect()
    }

    fn texts(source: &str) -> Vec<&str> {
        Lexer::new(source)
            .map(|token| {
                let span = token.unwrap().span;
                &source[span.range()]
            })
            .collect()
    }

    fn error(source: &str) -> LexError {
        Lexer::new(source)
            .find_map(Result::err)
            .expect("the source has an error")
    }

    /// Returns the text of each token, and the text skipped by each error.
    fn outline(source: &str) -> Vec<Result<&str, &str>> {
        Lexer::new(source)
            .map(|token| match token {
                Ok(token) => Ok(&source[token.span.range()]),
                Err(error) => Err(&source[error.span.range()]),
            })
            .collect()
    }

    #[test]
    fn reads_nothing_from_empty_source() {
        assert_eq!(Lexer::new("").count(), 0);
    }

    #[test]
    fn reads_nothing_from_gaps_and_comments() {
        assert_eq!(Lexer::new(" \n\t(* a *) (* b (* c *) *)\r\n").count(), 0);
    }

    #[test]
    fn remembers_where_each_comment_is() {
        let mut lexer = Lexer::new("a (* x *) = (* y (* z *) *) b; (* broken");
        lexer.by_ref().for_each(drop);
        assert_eq!(
            lexer.comments(),
            [Span { start: 2, end: 9 }, Span { start: 12, end: 27 }]
        );
    }

    #[test]
    fn reads_a_rule() {
        assert_eq!(
            texts("rule = 'a', ? U+0041 ?, 3 * b;"),
            [
                "rule",
                "=",
                "'a'",
                ",",
                "? U+0041 ?",
                ",",
                "3",
                "*",
                "b",
                ";"
            ]
        );
    }

    #[test]
    fn reads_the_symbol_of_each_token() {
        let symbols: Vec<CommentlessSymbol> = Lexer::new("a = (/ b /);")
            .map(|token| token.unwrap().symbol)
            .collect();
        assert_eq!(symbols.len(), 6);
        assert!(matches!(symbols[0], CommentlessSymbol::MetaIdentifier(_)));
        assert!(matches!(symbols[3], CommentlessSymbol::MetaIdentifier(_)));
        for (index, symbol) in [
            (1, Symbol::Defining),
            (2, Symbol::StartOption),
            (4, Symbol::EndOption),
            (5, Symbol::Terminator),
        ] {
            assert_eq!(symbols[index], CommentlessSymbol::Symbol(symbol));
        }
    }

    #[test]
    fn spans_exclude_surrounding_gaps() {
        assert_eq!(spans("  a  =  b  "), [(2, 3), (5, 6), (8, 9)]);
    }

    #[test]
    fn spans_include_gaps_inside_a_token() {
        assert_eq!(
            texts("syntax rule = 1 2;"),
            ["syntax rule", "=", "1 2", ";"]
        );
    }

    #[test]
    fn skips_comments_between_tokens() {
        assert_eq!(
            texts("a(*x*)=(* y (* z *) *)b;(* end *)"),
            ["a", "=", "b", ";"]
        );
    }

    #[test]
    fn ends_an_identifier_at_a_comment() {
        assert_eq!(texts("a (* x *) b"), ["a", "b"]);
    }

    #[test]
    fn splits_two_character_symbols_at_a_gap() {
        assert_eq!(texts("( * ) (/ /)"), ["(", "*", ")", "(/", "/)"]);
    }

    #[test]
    fn reads_a_group_and_an_end_comment_symbol_from_a_spaced_star() {
        assert_eq!(outline("( *)"), [Ok("("), Err("*)")]);
        assert_eq!(
            error("( *)").kind,
            LexErrorKind::CommentlessSymbol(
                ParseCommentlessSymbolError::UnmatchedEndCommentSymbol
            )
        );
    }

    #[test]
    fn rejects_an_unmatched_end_comment_symbol() {
        assert_eq!(
            error("a *) b"),
            LexError {
                kind: LexErrorKind::CommentlessSymbol(
                    ParseCommentlessSymbolError::UnmatchedEndCommentSymbol
                ),
                span: Span { start: 2, end: 4 },
            }
        );
    }

    #[test]
    fn rejects_an_unclosed_comment_up_to_the_end() {
        assert_eq!(
            error("a = b; (* note"),
            LexError {
                kind: LexErrorKind::BracketedTextualComment(
                    ParseBracketedTextualCommentError::NoEndCommentSymbol
                ),
                span: Span { start: 7, end: 14 },
            }
        );
    }

    #[test]
    fn rejects_an_invalid_character_with_its_whole_span() {
        assert_eq!(
            error("a = é;"),
            LexError {
                kind: LexErrorKind::CommentlessSymbol(
                    ParseCommentlessSymbolError::InvalidCharacter('é')
                ),
                span: Span { start: 4, end: 6 },
            }
        );
    }

    #[test]
    fn rejects_a_lone_carriage_return() {
        assert_eq!(error("a\r= b;").span, Span { start: 1, end: 2 });
    }

    #[test]
    fn reads_tokens_before_an_error() {
        let mut lexer = Lexer::new("a = @");
        assert!(lexer.next().is_some_and(|token| token.is_ok()));
        assert!(lexer.next().is_some_and(|token| token.is_ok()));
        assert!(lexer.next().is_some_and(|token| token.is_err()));
    }

    #[test]
    fn goes_on_after_a_character_it_cannot_read() {
        assert_eq!(
            outline("@ a é *) b"),
            [Err("@"), Ok("a"), Err("é"), Err("*)"), Ok("b")]
        );
    }

    #[test]
    fn collects_every_error_in_a_grammar() {
        let errors: Vec<&str> =
            outline("a = @;\nb = 'x\nc = ? x ?;\nd = 4294967296;")
                .into_iter()
                .filter_map(Result::err)
                .collect();
        assert_eq!(errors, ["@", "'x", "? x ?", "4294967296"]);
    }

    #[test]
    fn goes_on_after_an_integer_that_is_too_large() {
        assert_eq!(
            outline("4294 967 296 * a"),
            [Err("4294 967 296"), Ok("*"), Ok("a")]
        );
    }

    #[test]
    fn goes_on_after_the_closing_quote_of_a_broken_terminal_string() {
        assert_eq!(
            outline("a = 'x\ty', b;"),
            [Ok("a"), Ok("="), Err("'x\ty'"), Ok(","), Ok("b"), Ok(";")]
        );
        assert_eq!(
            outline("a = '', b;"),
            [Ok("a"), Ok("="), Err("''"), Ok(","), Ok("b"), Ok(";")]
        );
    }

    #[test]
    fn goes_on_at_the_next_line_after_an_unclosed_terminal_string() {
        assert_eq!(
            outline("a = 'x, b;\nc;"),
            [Ok("a"), Ok("="), Err("'x, b;"), Ok("c"), Ok(";")]
        );
        assert_eq!(
            outline("a = \"x\r\nc;"),
            [Ok("a"), Ok("="), Err("\"x\r"), Ok("c"), Ok(";")]
        );
    }

    #[test]
    fn goes_on_after_the_closing_symbol_of_a_broken_special_sequence() {
        assert_eq!(
            outline("a = ? x ?, ? U+D800 ?, b;"),
            [
                Ok("a"),
                Ok("="),
                Err("? x ?"),
                Ok(","),
                Err("? U+D800 ?"),
                Ok(","),
                Ok("b"),
                Ok(";")
            ]
        );
    }

    #[test]
    fn stops_at_an_unclosed_special_sequence() {
        assert_eq!(outline("a = ? x"), [Ok("a"), Ok("="), Err("? x")]);
    }

    #[test]
    fn goes_on_after_the_end_of_a_broken_comment() {
        assert_eq!(outline("(* don't *) a"), [Err("(* don't *)"), Ok("a")]);
        assert_eq!(
            outline("(* (* é *) *) a"),
            [Err("(* (* é *) *)"), Ok("a")]
        );
    }

    #[test]
    fn stops_at_an_unclosed_comment() {
        assert_eq!(outline("a (* b (* c *)"), [Ok("a"), Err("(* b (* c *)")]);
    }

    #[test]
    fn ends_after_the_last_error() {
        let mut lexer = Lexer::new("@");
        assert!(lexer.next().is_some_and(|token| token.is_err()));
        assert!(lexer.next().is_none());
        assert!(lexer.next().is_none());
    }

    #[test]
    fn error_displays_the_message_of_what_failed() {
        assert_eq!(error("*)").to_string(), "`*)` without a matching `(*`");
        assert_eq!(
            error("(* a").to_string(),
            "EBNF comment has no closing `*)`"
        );
    }

    #[test]
    fn error_gives_the_source_of_what_failed() {
        let error = error("(* don't *)");
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(ParseTerminalStringError::NoClosingQuote.to_string())
        );
    }
}
