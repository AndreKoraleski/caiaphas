//! The lexer, the mechanism to turn the grammar source into tokens.

#![cfg_attr(not(test), expect(dead_code, reason = "no parser exists to read"))]

mod bracketed_textual_comment;
mod commentless_symbol;
mod gap_separator;
mod terminal_character;
mod token;

use std::{error::Error, fmt};

use bracketed_textual_comment::ParseBracketedTextualCommentError;
use commentless_symbol::{CommentlessSymbol, ParseCommentlessSymbolError};
use token::Token;

use crate::span::Span;

/// Reads the tokens of a grammar one at a time, skipping gaps and comments.
///
/// It stops after the first error, so every later call to `next` returns
/// `None`.
#[derive(Debug, Clone)]
pub(crate) struct Lexer<'a> {
    source: &'a str,
    position: usize,
    failed: bool,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            failed: false,
        }
    }

    /// Moves past the gaps and comments at the current position.
    fn skip_gaps_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            self.position += gap_separator::length(self.rest());
            if !self.rest().starts_with("(*") {
                return Ok(());
            }
            let length = bracketed_textual_comment::length(self.rest())
                .map_err(|error| {
                    self.error(LexErrorKind::BracketedTextualComment(error))
                })?;
            self.position += length;
        }
    }

    fn read_token(&mut self) -> Result<Token, LexError> {
        let (symbol, length) =
            CommentlessSymbol::read(self.rest()).map_err(|error| {
                self.error(LexErrorKind::CommentlessSymbol(error))
            })?;
        let span = Span {
            start: self.position,
            end: self.position + length,
        };
        self.position = span.end;
        Ok(Token { symbol, span })
    }

    fn rest(&self) -> &'a str {
        &self.source[self.position..]
    }

    /// Returns an error whose span is the character at the current position.
    fn error(&self, kind: LexErrorKind) -> LexError {
        let length = self.rest().chars().next().map_or(0, char::len_utf8);
        LexError {
            kind,
            span: Span {
                start: self.position,
                end: self.position + length,
            },
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Result<Token, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let token = self
            .skip_gaps_and_comments()
            .and_then(|()| {
                if self.rest().is_empty() {
                    Ok(None)
                } else {
                    self.read_token().map(Some)
                }
            })
            .transpose();
        self.failed = matches!(token, Some(Err(_)));
        token
    }
}

/// Error returned when the source of a grammar cannot be split into tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LexError {
    pub(crate) kind: LexErrorKind,
    /// The first character of the token or comment that could not be read.
    pub(crate) span: Span,
}

/// What went wrong in a [`LexError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LexErrorKind {
    CommentlessSymbol(ParseCommentlessSymbolError),
    BracketedTextualComment(ParseBracketedTextualCommentError),
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LexErrorKind::CommentlessSymbol(error) => error.fmt(f),
            LexErrorKind::BracketedTextualComment(error) => error.fmt(f),
        }
    }
}

impl Error for LexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            LexErrorKind::CommentlessSymbol(error) => error.source(),
            LexErrorKind::BracketedTextualComment(error) => error.source(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{
        commentless_symbol::terminal_string::ParseTerminalStringError,
        terminal_character::Symbol,
    };

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
                &source[span.start..span.end]
            })
            .collect()
    }

    fn error(source: &str) -> LexError {
        Lexer::new(source)
            .find_map(Result::err)
            .expect("the source has an error")
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
                span: Span { start: 2, end: 3 },
            }
        );
    }

    #[test]
    fn rejects_an_unclosed_comment_at_its_start() {
        assert_eq!(
            error("a = b; (* note"),
            LexError {
                kind: LexErrorKind::BracketedTextualComment(
                    ParseBracketedTextualCommentError::NoClosingSymbol
                ),
                span: Span { start: 7, end: 8 },
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
    fn stops_after_an_error() {
        let mut lexer = Lexer::new("@ a b c");
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
