//! Names turn the meta identifiers and words of a grammar into Rust
//! identifiers that follow Rust's naming conventions.

use proc_macro2::{Ident, Span};

use crate::lexer::MetaIdentifier;

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

/// Returns the words of `meta_identifier`, lowercase, split at its gaps and
/// where its case changes, so `syntax rule` and `syntaxRule` both give
/// `syntax` and `rule`.
pub(crate) fn words(meta_identifier: &MetaIdentifier) -> Vec<String> {
    meta_identifier.words().flat_map(split_case).collect()
}

/// Splits `word` where a capital letter starts a new word, so `JSONValue`
/// gives `json` and `value`.
fn split_case(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (index, &c) in chars.iter().enumerate() {
        let previous = index.checked_sub(1).map(|index| chars[index]);
        let next = chars.get(index + 1);
        let starts_word = c.is_ascii_uppercase()
            && previous.is_some_and(|previous| {
                !previous.is_ascii_uppercase()
                    || next.is_some_and(char::is_ascii_lowercase)
            });
        if starts_word {
            words.push(current.to_ascii_lowercase());
            current.clear();
        }
        current.push(c);
    }
    words.push(current.to_ascii_lowercase());
    words
}

/// Returns the name of a type or variant, such as `SyntaxRule` for the words
/// `syntax` and `rule`.
pub(crate) fn type_name(words: &[String]) -> Ident {
    let name: String = words
        .iter()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect();
    ident(&name)
}

/// Returns the name of a field, such as `syntax_rule` for the words `syntax`
/// and `rule`.
pub(crate) fn field_name(words: &[String]) -> Ident {
    ident(&words.join("_"))
}

/// Returns the name of the variant for the terminal string `text`, when the
/// text is a word that a name can be made of.
pub(crate) fn word_variant(text: &str) -> Option<Ident> {
    let mut chars = text.chars();
    let is_word = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric());
    is_word.then(|| type_name(&split_case(text)))
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

    fn words_of(written: &str) -> Vec<String> {
        let (meta_identifier, _) = MetaIdentifier::read(written).unwrap();
        words(&meta_identifier)
    }

    #[test]
    fn splits_words_at_gaps_and_case() {
        assert_eq!(words_of("syntax rule"), ["syntax", "rule"]);
        assert_eq!(words_of("syntaxRule"), ["syntax", "rule"]);
        assert_eq!(words_of("SyntaxRule"), ["syntax", "rule"]);
        assert_eq!(words_of("JSONValue"), ["json", "value"]);
        assert_eq!(words_of("utf8 Byte"), ["utf8", "byte"]);
        assert_eq!(words_of("json"), ["json"]);
    }

    #[test]
    fn names_types_and_fields_the_rust_way() {
        let words = words_of("syntax rule");
        assert_eq!(type_name(&words).to_string(), "SyntaxRule");
        assert_eq!(field_name(&words).to_string(), "syntax_rule");
        let words = words_of("hex digit 2");
        assert_eq!(type_name(&words).to_string(), "HexDigit2");
        assert_eq!(field_name(&words).to_string(), "hex_digit_2");
    }

    #[test]
    fn makes_keywords_raw_and_moves_names_that_cannot_be() {
        assert_eq!(field_name(&words_of("type")).to_string(), "r#type");
        assert_eq!(field_name(&words_of("self")).to_string(), "self_");
        assert_eq!(field_name(&words_of("span")).to_string(), "span_");
        assert_eq!(type_name(&words_of("self")).to_string(), "Self_");
        assert_eq!(type_name(&words_of("type")).to_string(), "Type");
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
