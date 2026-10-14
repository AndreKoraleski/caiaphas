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

mod gaps;
mod integer;
mod meta_identifier;
mod special_sequence;
mod terminal_character;
mod terminal_string;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
