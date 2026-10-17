//! A parse that recovers goes on past the errors in its input, so it tells all
//! of them.

use crate::Error;

/// What a parse that goes on past its errors gives.
///
/// A rule written with `(* recover *)` that fails partway lets the parse skip
/// to its next match. The parts of the input that still match then make the
/// value, and `errors` tells what was skipped.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Recovered<T> {
    /// The value of the input with each error skipped, when the rest matches.
    pub value: Option<T>,
    /// Every error found, in the order of the input. Empty when the input
    /// matches as it is.
    pub errors: Vec<Error>,
}

impl<T> Recovered<T> {
    pub(crate) fn new(value: Option<T>, errors: Vec<Error>) -> Self {
        Self { value, errors }
    }
}
