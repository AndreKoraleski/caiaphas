//! EBNF parser generator for Rust that detects grammar ambiguities and
//! suggests fixes.

mod error;
mod span;

#[doc(hidden)]
pub mod __private;

pub use caiaphas_macros::grammar;
pub use error::Error;
pub use span::Span;
