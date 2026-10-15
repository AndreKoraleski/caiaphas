//! EBNF parser generator for Rust that detects grammar ambiguities and
//! suggests fixes
//!
//! # Examples
//!
//! ```
//! use caiaphas::add;
//!
//! assert_eq!(add(2, 2), 4);
//! ```

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "nothing outside the crate calls the parser yet"
    )
)]

/// Adds two numbers together.
///
/// # Examples
///
/// ```
/// use caiaphas::add;
///
/// assert_eq!(add(2, 2), 4);
/// ```
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

mod analysis;
mod diagnostic;
mod dictionary;
mod expectation;
mod ir;
mod lexer;
mod parser;
mod span;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
