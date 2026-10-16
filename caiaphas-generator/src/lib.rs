//! Reads ISO/IEC 14977 EBNF grammars for caiaphas, finds their problems and
//! generates their parsers.
//!
//! Depend on the `caiaphas` crate instead. This one does the work behind its
//! `grammar!` macro and the `caiaphas` command, and its API can change in any
//! release. The [`findings`] page explains every finding.

mod analysis;
mod annotation;
mod compile;
mod diagnostic;
mod dictionary;
mod explain;
mod generate;
mod ir;
mod lexer;
mod parser;
mod span;
mod spelling;

pub use explain::{codes, explain, findings, similar_code};
pub use generate::{Module, Options, Report, check, generate};
