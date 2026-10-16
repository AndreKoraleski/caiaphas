//! EBNF parser generator for Rust that detects grammar ambiguities and
//! suggests fixes.
//!
//! Write a grammar in ISO/IEC 14977 EBNF and generate its module with
//! [`grammar!`], giving its path from the root of your crate. The module has a
//! type for each rule, and the type of each start rule parses a whole input. A
//! start rule is one that no other rule uses:
//!
//! ```
//! caiaphas::grammar!(mod json = "tests/grammars/json.ebnf");
//!
//! use json::{Json, Value};
//!
//! let document: Json = r#"{"name": "caiaphas"}"#.parse()?;
//! let Value::Object(object) = &document.value else {
//!     unreachable!("the document is an object");
//! };
//! assert_eq!(object.member[0].string.text(), r#""name""#);
//! # Ok::<(), caiaphas::Error>(())
//! ```
//!
//! The [`tutorial`] goes through grammars, the types generated from them,
//! errors, how a parser chooses between the ways an input reads, and the
//! checks caiaphas runs on a grammar. Every finding those checks report is
//! listed on the `findings` page of the [caiaphas-generator docs][generator],
//! and `caiaphas explain` from the `caiaphas-cli` crate prints the same.
//!
//! [generator]: https://docs.rs/caiaphas-generator

mod error;
mod span;

#[doc(hidden)]
pub mod __private;
pub mod tutorial;

pub use caiaphas_macros::grammar;
pub use error::{Error, Expected};
pub use span::Span;
