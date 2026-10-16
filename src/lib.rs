//! EBNF parser generator for Rust that detects grammar ambiguities and
//! suggests fixes.
//!
//! Write a grammar in ISO/IEC 14977 EBNF and generate its module with
//! [`grammar!`], giving its path from the root of your crate. The module has a
//! type for each rule, and the type of each start rule, which no other rule
//! uses, parses a whole input:
//!
//! ```
//! caiaphas::grammar!(mod json = "tests/grammars/json.ebnf");
//!
//! use json::{Json, Value};
//!
//! let document = Json::parse(r#"{"name": "caiaphas"}"#)?;
//! let Value::Object(object) = &document.value else {
//!     unreachable!("the document is an object");
//! };
//! assert_eq!(object.member[0].string.text(), r#""name""#);
//! # Ok::<(), caiaphas::Error>(())
//! ```
//!
//! # Types
//!
//! Each rule becomes a type named after it, so `syntax rule` becomes
//! `SyntaxRule`.
//!
//! - A rule whose alternatives are each another rule or a word becomes an enum
//!   with a variant for each, such as `Value::Object` and `Value::True`.
//! - Any other rule becomes a struct with a field for each rule it holds,
//!   named after that rule. A field holds `T`, `Option<T>` or `Vec<T>`, as
//!   many times as the rule can hold it, and a type that could contain itself
//!   holds itself in a `Box`.
//! - Every type has `text()`, the text its rule matched, and every struct has
//!   `span()`, which tells where. Types own what they hold, so they need no
//!   lifetimes.
//! - A rule written right after `(* hide *)`, such as whitespace, gets no type
//!   and no field.
//!
//! # Grammars
//!
//! A grammar works on characters, so it describes its whitespace too. A
//! special sequence holds one code point, such as `? U+00E9 ?`, or a range of
//! them, such as `? U+0041..U+005A ?`.
//!
//! Where a choice has more than one way to go, the parser takes the
//! alternative that matches the most text, and the first written of those that
//! tie. Optional and repeated sequences take as much as they can.
//!
//! # Problems
//!
//! Before generating, the macro looks for what would make the parser fail, or
//! not do what the grammar seems to mean, and stops with a suggestion for each
//! problem. When a warning describes what you intend, write an expectation
//! right before its rule, with the code its suggestion gives and a reason:
//!
//! ```text
//! (* expect tied-branches: a keyword is never an identifier *)
//! token = keyword | identifier;
//! ```

mod error;
mod span;

#[doc(hidden)]
pub mod __private;

pub use caiaphas_macros::grammar;
pub use error::Error;
pub use span::Span;
