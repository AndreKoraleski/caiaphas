//! Names turn the meta identifiers and words of a grammar into Rust
//! identifiers that follow Rust's naming conventions.
//!
//! A name is split into words at its gaps and where its case changes, so
//! `syntax rule`, `syntaxRule` and `SyntaxRule` all give the same names.

use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, Span};

use crate::ir::Rule;

/// Words Rust reserves. An identifier can only be one when raw.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const",
    "continue", "do", "dyn", "else", "enum", "extern", "false", "final", "fn",
    "for", "gen", "if", "impl", "in", "let", "loop", "macro", "match", "mod",
    "move", "mut", "override", "priv", "pub", "ref", "return", "static",
    "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized",
    "use", "virtual", "where", "while", "yield",
];

/// Names an identifier cannot be even when raw. `span` is among them because
/// the generated types keep where each rule matched in a field of that name.
const TAKEN: &[&str] = &["crate", "self", "Self", "super", "span"];

/// Returns the name of `rule` as written, with single spaces between its
/// words.
pub(crate) fn spaced(rule: &Rule) -> String {
    rule.meta_identifier.spaced()
}

/// Returns the name of a type or variant, such as `SyntaxRule` for
/// `syntax rule`.
pub(crate) fn type_name(written: &str) -> Ident {
    ident(&written.to_upper_camel_case())
}

/// Returns the name of a field, such as `syntax_rule` for `syntax rule`.
pub(crate) fn field_name(written: &str) -> Ident {
    ident(&written.to_snake_case())
}

/// The name of each character of punctuation in the name of a variant.
const PUNCTUATION: &[(char, &str)] = &[
    ('!', "Bang"),
    ('"', "Quote"),
    ('#', "Hash"),
    ('$', "Dollar"),
    ('%', "Percent"),
    ('&', "Ampersand"),
    ('\'', "Apostrophe"),
    ('(', "OpenParen"),
    (')', "CloseParen"),
    ('*', "Star"),
    ('+', "Plus"),
    (',', "Comma"),
    ('-', "Minus"),
    ('.', "Dot"),
    ('/', "Slash"),
    (':', "Colon"),
    (';', "Semicolon"),
    ('<', "Less"),
    ('=', "Equals"),
    ('>', "Greater"),
    ('?', "Question"),
    ('@', "At"),
    ('[', "OpenBracket"),
    ('\\', "Backslash"),
    (']', "CloseBracket"),
    ('^', "Caret"),
    ('_', "Underscore"),
    ('`', "Backtick"),
    ('{', "OpenBrace"),
    ('|', "Pipe"),
    ('}', "CloseBrace"),
    ('~', "Tilde"),
];

/// Returns the name of the variant for the terminal string `text`, when every
/// part of it has one: each word in upper camel case, and each character of
/// punctuation by its name, so `<=` gives `LessEquals` and `not in` gives
/// `NotIn`. A word that starts with a digit has none, so no variant is named
/// after a number.
pub(crate) fn terminal_variant(text: &str) -> Option<Ident> {
    let mut name = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == ' ' {
            rest = &rest[1..];
            continue;
        }
        if c.is_ascii_alphanumeric() {
            let end = rest
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            if c.is_ascii_digit() {
                return None;
            }
            name.push_str(&rest[..end].to_upper_camel_case());
            rest = &rest[end..];
            continue;
        }
        let &(_, part) = PUNCTUATION.iter().find(|&&(known, _)| known == c)?;
        name.push_str(part);
        rest = &rest[c.len_utf8()..];
    }
    (!name.is_empty()).then(|| ident(&name))
}

fn ident(name: &str) -> Ident {
    if TAKEN.contains(&name) {
        Ident::new(&format!("{name}_"), Span::call_site())
    } else if KEYWORDS.contains(&name) {
        Ident::new_raw(name, Span::call_site())
    } else {
        Ident::new(name, Span::call_site())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(written: &str) -> (String, String) {
        (
            type_name(written).to_string(),
            field_name(written).to_string(),
        )
    }

    #[test]
    fn splits_words_at_gaps_and_case() {
        let syntax_rule = ("SyntaxRule".to_owned(), "syntax_rule".to_owned());
        assert_eq!(names("syntax rule"), syntax_rule);
        assert_eq!(names("syntaxRule"), syntax_rule);
        assert_eq!(names("SyntaxRule"), syntax_rule);
        assert_eq!(names("syntax\r\n  rule"), syntax_rule);
        assert_eq!(
            names("JSONValue"),
            ("JsonValue".to_owned(), "json_value".to_owned())
        );
        assert_eq!(
            names("utf8 Byte"),
            ("Utf8Byte".to_owned(), "utf8_byte".to_owned())
        );
        assert_eq!(
            names("hex digit 2"),
            ("HexDigit2".to_owned(), "hex_digit_2".to_owned())
        );
    }

    #[test]
    fn makes_keywords_raw_and_moves_names_that_cannot_be() {
        assert_eq!(field_name("type").to_string(), "r#type");
        assert_eq!(field_name("self").to_string(), "self_");
        assert_eq!(field_name("span").to_string(), "span_");
        assert_eq!(type_name("self").to_string(), "Self_");
        assert_eq!(type_name("type").to_string(), "Type");
    }

    #[test]
    fn names_variants_after_words_and_punctuation() {
        let name = |text| terminal_variant(text).map(|name| name.to_string());
        assert_eq!(name("true").as_deref(), Some("True"));
        assert_eq!(name("isNull").as_deref(), Some("IsNull"));
        assert_eq!(name("not in").as_deref(), Some("NotIn"));
        assert_eq!(name("+").as_deref(), Some("Plus"));
        assert_eq!(name("<=").as_deref(), Some("LessEquals"));
        assert_eq!(name("is_a").as_deref(), Some("IsUnderscoreA"));
        assert_eq!(name("x2").as_deref(), Some("X2"));
        assert_eq!(name("2x"), None);
        assert_eq!(name("1"), None);
        assert_eq!(name("é"), None);
        assert_eq!(name(" "), None);
        assert_eq!(name(""), None);
    }
}
