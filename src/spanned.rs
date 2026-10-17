//! Spanned values keep where a rule matched the text they were read from.

use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};

use crate::Span;

/// A value read from the text a rule matched, with where the rule matched it.
/// A rule written with `(* as caiaphas::Spanned<T> *)` gives one, so a program
/// can point at a value it finds wrong, such as a name it does not know.
///
/// Otherwise it reads as the value it holds: it derefs to it, and compares,
/// hashes and displays as it does, wherever it is.
///
/// ```
/// caiaphas::grammar!(mod names {
///     "names = name, {' ', name};
///      (* as caiaphas::Spanned<String> *) name = ? /[a-z]+/ ?;"
/// });
///
/// let parsed = names::Names::parse("ab cd").unwrap();
/// assert_eq!(parsed.name[1], "cd".to_owned());
/// assert_eq!(parsed.name[1].len(), 2);
/// assert_eq!(parsed.name[1].span().column(), 4);
/// ```
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    value: T,
    span: Span,
}

impl<T> Spanned<T> {
    pub(crate) fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }

    /// Returns where the rule matched the text of the value.
    #[must_use]
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the text the value was read from.
    #[must_use]
    pub fn text(&self) -> &str {
        self.span.as_str()
    }

    /// Returns the value, without where it was read.
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: PartialEq> PartialEq<T> for Spanned<T> {
    fn eq(&self, other: &T) -> bool {
        self.value == *other
    }
}

impl<T: Eq> Eq for Spanned<T> {}

impl<T: Hash> Hash for Spanned<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T: fmt::Display> fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}
