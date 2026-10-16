//! Reads ISO/IEC 14977 EBNF grammars for caiaphas, finds their problems and
//! generates their parsers.
//!
//! The `caiaphas` crate is the one to depend on. This one does the work behind
//! its `grammar!` macro.

mod analysis;
mod annotation;
mod compile;
mod diagnostic;
mod dictionary;
mod generate;
mod ir;
mod lexer;
mod parser;
mod span;

pub use generate::{Module, generate};
