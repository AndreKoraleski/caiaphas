//! Names turn the meta identifiers and words of a grammar into Rust
//! identifiers that follow Rust's naming conventions.
//!
//! A name is split into words at its gaps and where its case changes, so
//! `syntax rule`, `syntaxRule` and `SyntaxRule` all give the same names.

use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, Span};

use crate::ir::Rule;

/// Words Rust reserves, which an identifier can only be when raw.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const",
    "continue", "do", "dyn", "else", "enum", "extern", "false", "final", "fn",
    "for", "gen", "if", "impl", "in", "let", "loop", "macro", "match", "mod",
    "move", "mut", "override", "priv", "pub", "ref", "return", "static",
    "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized",
    "use", "virtual", "where", "while", "yield",
];

/// Names an identifier cannot be even when raw, along with `span`, which the
/// generated types keep for where each rule matched.
const TAKEN: &[&str] = &["crate", "self", "Self", "super", "span"];

/// Returns the name of `rule` as written, with single spaces between its
/// words.
pub(crate) fn spaced(rule: &Rule) -> String {
    rule.meta_identifier.words().collect::<Vec<_>>().join(" ")
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

/// Returns the name of the variant for the terminal string `text`, when the
/// text is a word that a name can be made of.
pub(crate) fn word_variant(text: &str) -> Option<Ident> {
    let mut chars = text.chars();
    let is_word = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric());
    is_word.then(|| type_name(text))
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
    fn names_variants_after_words() {
        let name = |text| word_variant(text).map(|name| name.to_string());
        assert_eq!(name("true").as_deref(), Some("True"));
        assert_eq!(name("isNull").as_deref(), Some("IsNull"));
        assert_eq!(name("+"), None);
        assert_eq!(name("2x"), None);
        assert_eq!(name(""), None);
    }
}
