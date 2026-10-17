//! Annotations are comments right before a syntax rule that tell caiaphas
//! something about the rule. Other EBNF tools read them as comments.
//!
//! ```text
//! (* expect tied-branches: a keyword is never a name *)
//! word = keyword | name;
//! ```
//!
//! An annotation is `hide`, `inline`, `greedy`, `recover`, `as` and a Rust
//! type, or `expect` with a finding's code, a colon and a reason. A type is a
//! primitive or a path whose last part starts with a capital, so a comment
//! such as `(* as such *)` stays a comment. Each comment holds one annotation,
//! and a rule can have several. The standard reads quotes in a comment as
//! terminal strings, so a reason cannot hold a lone `'` or `"`.

use std::fmt;

use crate::{explain, span::Span, spelling};

/// An expectation that a syntax rule has a finding with a given code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expectation {
    /// The code of the finding, in kebab case.
    pub(crate) code: String,
    /// Why the finding is intended.
    pub(crate) reason: String,
    /// The comment it is written in.
    pub(crate) span: Span,
}

/// What a comment right before a syntax rule tells caiaphas about the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Annotation {
    Expectation(Expectation),
    /// Leaves the rule out of the types generated for the grammar.
    Hide,
    /// Makes the rule take each way through it as it comes to it.
    Greedy,
    /// Lets a parse that recovers skip the rule ahead to its next match where
    /// it fails partway.
    Recover,
    /// Gives the rules that hold the rule what it holds, instead of its type.
    Inline,
    /// Makes the rule's type this Rust type, read from the text it matched.
    Converts(String),
}

/// What a comment holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Comment {
    /// Holds text for people only.
    Ordinary,
    Annotation(Annotation),
    /// Text that reads as an annotation written wrongly.
    Malformed(Malformed),
}

/// How an annotation is written wrongly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Malformed {
    /// An annotation of a few words, such as `hide`, holds more than them.
    NotAlone(&'static str),
    /// An expectation has no code.
    NoCode,
    /// An expectation has no `:` after its code.
    NoColon,
    /// The code of an expectation is not in kebab case, though maybe the
    /// `similar` one was meant.
    NotKebabCase { similar: Option<&'static str> },
    /// An expectation has no reason after its `:`.
    NoReason,
    /// An `as` names no type.
    NoType,
    /// The comment is one word written almost as the annotation `meant`.
    Misspelled { meant: &'static str },
    /// The comment holds several annotations apart by commas.
    Several,
}

impl Comment {
    /// Reads the comment `text`, `(*` and `*)` included, found at `span`.
    pub(crate) fn read(text: &str, span: Span) -> Self {
        let content = text
            .strip_prefix("(*")
            .and_then(|text| text.strip_suffix("*)"))
            .unwrap_or(text)
            .trim_matches(is_gap);
        let parts: Vec<&str> = content.split(',').collect();
        let several = parts.len() > 1
            && parts.iter().all(|part| {
                let part = format!("(*{part}*)");
                matches!(Self::read(&part, span), Self::Annotation(_))
            });
        if several {
            return Self::Malformed(Malformed::Several);
        }
        if let Some(rest) = after_word(content, "as") {
            let rest = rest.trim_matches(is_gap);
            if rest.is_empty() {
                return Self::Malformed(Malformed::NoType);
            }
            if is_type(rest) {
                return Self::Annotation(Annotation::Converts(
                    rest.to_owned(),
                ));
            }
        }
        let alone = [
            ("hide", after_word(content, "hide"), Annotation::Hide),
            ("inline", after_word(content, "inline"), Annotation::Inline),
            ("greedy", after_word(content, "greedy"), Annotation::Greedy),
            (
                "recover",
                after_word(content, "recover"),
                Annotation::Recover,
            ),
        ];
        for (name, rest, annotation) in alone {
            if let Some(rest) = rest {
                return if rest.is_empty() {
                    Self::Annotation(annotation)
                } else {
                    Self::Malformed(Malformed::NotAlone(name))
                };
            }
        }
        let Some(rest) = after_word(content, "expect") else {
            return misspelled(content);
        };
        match expectation(rest, span) {
            Ok(expectation) => {
                Self::Annotation(Annotation::Expectation(expectation))
            }
            Err(malformed) => Self::Malformed(malformed),
        }
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAlone("hide") => {
                f.write_str("a rule is hidden with `(* hide *)` alone")
            }
            Self::NotAlone("recover") => {
                f.write_str("a rule recovers with `(* recover *)` alone")
            }
            Self::NotAlone(name) => {
                write!(f, "a rule is made {name} with `(* {name} *)` alone")
            }
            Self::NoCode => f.write_str("an expectation names no finding"),
            Self::NoColon => f.write_str(
                "an expectation needs a `:` between its code and its reason",
            ),
            Self::NotKebabCase { .. } => {
                f.write_str("the code of an expectation is in kebab case")
            }
            Self::NoReason => {
                f.write_str("an expectation needs a reason after its `:`")
            }
            Self::NoType => f.write_str("`(* as *)` names no type"),
            Self::Misspelled { meant } => write!(
                f,
                "this comment reads like `(* {meant} *)` written wrongly"
            ),
            Self::Several => {
                f.write_str("a comment holds one annotation, not several")
            }
        }
    }
}

impl Malformed {
    /// Returns how to write the annotation.
    pub(crate) fn help(self) -> String {
        const EXAMPLE: &str =
            "`(* expect tied-branches: a keyword is never a name *)`";
        match self {
            Self::NotAlone(name) => format!(
                "write `(* {name} *)` in a comment of its own, and the rest \
                 in another"
            ),
            Self::NoCode => format!(
                "name the finding it expects by its code, as in {EXAMPLE}; \
                 `caiaphas explain` lists every code"
            ),
            Self::NoColon => {
                "write it as `(* expect code: reason *)`".to_owned()
            }
            Self::NotKebabCase {
                similar: Some(similar),
            } => format!("write `{similar}`"),
            Self::NotKebabCase { similar: None } => {
                "write the code in lowercase words joined by `-`, as in \
                 `tied-branches`"
                    .to_owned()
            }
            Self::NoReason => {
                format!("say why the finding is intended, as in {EXAMPLE}")
            }
            Self::NoType => {
                "name the Rust type that reads the rule's text, as in \
                 `(* as u16 *)`"
                    .to_owned()
            }
            Self::Misspelled { meant: "expect" } => {
                "write `(* expect code: reason *)` if an expectation was \
                 meant, or more words for an ordinary comment"
                    .to_owned()
            }
            Self::Several => "write each annotation in a comment of its own, \
                              as in `(* hide *) (* inline *)`"
                .to_owned(),
            Self::Misspelled { meant } => format!(
                "write `(* {meant} *)` if that was meant, or more words for \
                 an ordinary comment"
            ),
        }
    }
}

/// Returns the expectation written `rest` after `expect` in the comment at
/// `span`, or how it is written wrongly.
fn expectation(rest: &str, span: Span) -> Result<Expectation, Malformed> {
    if rest.trim_matches(is_gap).is_empty() {
        return Err(Malformed::NoCode);
    }
    let (code, reason) = rest.split_once(':').ok_or(Malformed::NoColon)?;
    let code = code.trim_matches(is_gap);
    let reason = reason.trim_matches(is_gap);
    if code.is_empty() {
        return Err(Malformed::NoCode);
    }
    if !is_kebab_case(code) {
        let similar = explain::similar_code(&kebab_case(code));
        return Err(Malformed::NotKebabCase { similar });
    }
    if reason.is_empty() {
        return Err(Malformed::NoReason);
    }
    Ok(Expectation {
        code: code.to_owned(),
        reason: reason.to_owned(),
        span,
    })
}

/// Returns a comment of one word written almost as an annotation as a
/// misspelling of it, and any other as ordinary.
fn misspelled(content: &str) -> Comment {
    const WORDS: &[&str] = &["hide", "inline", "greedy", "recover", "expect"];
    if content.is_empty() || content.contains(is_gap) {
        return Comment::Ordinary;
    }
    match spelling::closest(content, WORDS.iter().copied()) {
        Some(meant) => Comment::Malformed(Malformed::Misspelled { meant }),
        None => Comment::Ordinary,
    }
}

/// Returns `code` in kebab case, as `TiedBranches` and `tied_branches` both
/// become `tied-branches`.
fn kebab_case(code: &str) -> String {
    let mut kebab = String::new();
    for c in code.chars() {
        if c.is_ascii_uppercase() && !kebab.is_empty() && !kebab.ends_with('-')
        {
            kebab.push('-');
        }
        match c {
            '_' | ' ' => kebab.push('-'),
            _ => kebab.push(c.to_ascii_lowercase()),
        }
    }
    kebab
}

/// Returns what follows `word` at the start of `content` when it is a word of
/// its own there, so `expect` is found in `expect:` but not in `expected`.
fn after_word<'a>(content: &'a str, word: &str) -> Option<&'a str> {
    let rest = content.strip_prefix(word)?;
    rest.chars()
        .next()
        .is_none_or(|c| is_gap(c) || c == ':')
        .then_some(rest)
}

/// Returns whether `c` can be part of a gap. A lexer only lets a carriage
/// return into a comment as part of a new line, so taking one on its own here
/// changes nothing.
fn is_gap(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C')
}

/// Returns whether `text` reads as a Rust type: a primitive, or a path whose
/// last part starts with a capital letter.
fn is_type(text: &str) -> bool {
    const PRIMITIVES: &[&str] = &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "f32", "f64", "bool", "char",
    ];
    // A generic type is a path to a type with types between `<` and `>`.
    if let Some((path, arguments)) =
        text.strip_suffix('>').and_then(|text| text.split_once('<'))
    {
        return is_type(path.trim_end()) && arguments_are_types(arguments);
    }
    let path = text.strip_prefix("::").unwrap_or(text);
    let parts: Vec<&str> = path.split("::").collect();
    let identifier = |part: &&str| {
        let mut chars = part.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if !parts.iter().all(identifier) {
        return false;
    }
    let last = parts[parts.len() - 1];
    parts.len() == 1 && PRIMITIVES.contains(&last)
        || last.starts_with(|c: char| c.is_ascii_uppercase())
}

/// Returns whether `arguments`, the inside of `<` and `>`, are types apart by
/// commas outside any `<` and `>` of their own.
fn arguments_are_types(arguments: &str) -> bool {
    let mut depth = 0_usize;
    let mut start = 0;
    let mut types = Vec::new();
    for (index, c) in arguments.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                types.push(&arguments[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    types.push(&arguments[start..]);
    types.iter().all(|ty| is_type(ty.trim_matches(is_gap)))
}

/// Returns whether `text` is lowercase words joined by `-`.
fn is_kebab_case(text: &str) -> bool {
    text.split('-').all(|word| {
        !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Comment {
        Comment::read(
            text,
            Span {
                start: 0,
                end: text.len(),
            },
        )
    }

    fn expectation(code: &str, reason: &str, length: usize) -> Comment {
        Comment::Annotation(Annotation::Expectation(Expectation {
            code: code.to_owned(),
            reason: reason.to_owned(),
            span: Span {
                start: 0,
                end: length,
            },
        }))
    }

    #[test]
    fn reads_an_expectation() {
        let text = "(* expect tied-branches: the first is meant *)";
        assert_eq!(
            read(text),
            expectation("tied-branches", "the first is meant", text.len())
        );
    }

    #[test]
    fn reads_an_expectation_over_several_lines() {
        let text = "(*\n  expect empty-rule :\n  kept for later\n*)";
        assert_eq!(
            read(text),
            expectation("empty-rule", "kept for later", text.len())
        );
    }

    #[test]
    fn keeps_colons_in_the_reason() {
        let text = "(* expect empty-rule: see: below *)";
        assert_eq!(
            read(text),
            expectation("empty-rule", "see: below", text.len())
        );
    }

    #[test]
    fn reads_other_comments_as_ordinary() {
        for text in [
            "(* a comment *)",
            "(**)",
            "(* expected to change *)",
            "(* expectation: none *)",
            "(* see expect below *)",
            "(* hidden for now *)",
            "(* hides the rest *)",
            "(* first, the header *)",
            "(* first matching rule *)",
            "(* match firstly *)",
        ] {
            assert_eq!(read(text), Comment::Ordinary, "{text:?}");
        }
    }

    #[test]
    fn reads_a_hide() {
        for text in ["(* hide *)", "(*hide*)", "(*\n  hide\n*)"] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Hide),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_a_hide_with_more_after_it() {
        for text in ["(* hide whitespace *)", "(* hide: whitespace *)"] {
            assert_eq!(
                read(text),
                Comment::Malformed(Malformed::NotAlone("hide")),
                "{text:?}"
            );
        }
    }

    #[test]
    fn reads_a_recover() {
        assert_eq!(
            read("(* recover *)"),
            Comment::Annotation(Annotation::Recover)
        );
        assert_eq!(
            read("(* recover lines *)"),
            Comment::Malformed(Malformed::NotAlone("recover"))
        );
        assert_eq!(
            read("(* recovr *)"),
            Comment::Malformed(Malformed::Misspelled { meant: "recover" })
        );
    }

    #[test]
    fn reads_a_greedy() {
        for text in ["(* greedy *)", "(*greedy*)", "(*\n  greedy\n*)"] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Greedy),
                "{text:?}"
            );
        }
        assert_eq!(
            read("(* greedy else *)"),
            Comment::Malformed(Malformed::NotAlone("greedy"))
        );
    }

    #[test]
    fn reads_an_inline() {
        for text in ["(* inline *)", "(*inline*)", "(*\n  inline\n*)"] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Inline),
                "{text:?}"
            );
        }
        assert_eq!(
            read("(* inline group *)"),
            Comment::Malformed(Malformed::NotAlone("inline"))
        );
    }

    #[test]
    fn reads_a_conversion_to_a_rust_type() {
        for (text, ty) in [
            ("(* as u16 *)", "u16"),
            ("(* as String *)", "String"),
            ("(*as std::net::Ipv4Addr*)", "std::net::Ipv4Addr"),
        ] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Converts(ty.to_owned())),
                "{text:?}"
            );
        }
        for text in ["(* as such *)", "(* as noted above *)", "(* as 1x *)"] {
            assert_eq!(read(text), Comment::Ordinary, "{text:?}");
        }
        assert_eq!(read("(* as *)"), Comment::Malformed(Malformed::NoType));
        for (text, ty) in [
            ("(* as Vec<u8> *)", "Vec<u8>"),
            (
                "(* as HashMap<String, Vec<u8>> *)",
                "HashMap<String, Vec<u8>>",
            ),
        ] {
            assert_eq!(
                read(text),
                Comment::Annotation(Annotation::Converts(ty.to_owned())),
                "{text:?}"
            );
        }
    }

    #[test]
    fn reads_several_annotations_in_one_comment_as_a_mistake() {
        assert_eq!(
            read("(* hide, as u8 *)"),
            Comment::Malformed(Malformed::Several)
        );
        assert_eq!(read("(* hide, then read *)"), Comment::Ordinary);
    }

    #[test]
    fn tells_how_an_expectation_is_written_wrongly() {
        let similar = Some("tied-branches");
        for (text, malformed) in [
            ("(* expect *)", Malformed::NoCode),
            ("(* expect : no code *)", Malformed::NoCode),
            ("(* expect: no code *)", Malformed::NoCode),
            ("(* expect tied-branches *)", Malformed::NoColon),
            ("(* expect tied-branches: *)", Malformed::NoReason),
            (
                "(* expect Tied-Branches: case *)",
                Malformed::NotKebabCase { similar },
            ),
            (
                "(* expect TiedBranches: camel *)",
                Malformed::NotKebabCase { similar },
            ),
            (
                "(* expect tied_branches: underscore *)",
                Malformed::NotKebabCase { similar },
            ),
            (
                "(* expect tied--branches: empty word *)",
                Malformed::NotKebabCase { similar },
            ),
            (
                "(* expect tied branches: space *)",
                Malformed::NotKebabCase { similar },
            ),
        ] {
            assert_eq!(read(text), Comment::Malformed(malformed), "{text:?}");
        }
    }

    #[test]
    fn reads_one_word_written_almost_as_an_annotation_as_a_misspelling() {
        for (text, meant) in [
            ("(* hidden *)", "hide"),
            ("(* Hide *)", "hide"),
            ("(* inlined *)", "inline"),
            ("(* greed *)", "greedy"),
        ] {
            assert_eq!(
                read(text),
                Comment::Malformed(Malformed::Misspelled { meant }),
                "{text:?}"
            );
        }
        for text in ["(* header *)", "(* TODO *)", "(* hidden for now *)"] {
            assert_eq!(read(text), Comment::Ordinary, "{text:?}");
        }
    }
}
