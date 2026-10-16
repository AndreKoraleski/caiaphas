//! Expands grammars in a crate that denies what strict crates deny, so the
//! code the macro generates builds in any of them.

#![deny(
    missing_docs,
    missing_debug_implementations,
    unreachable_pub,
    unused_qualifications,
    rust_2018_idioms,
    unused_lifetimes,
    trivial_casts,
    trivial_numeric_casts
)]
#![deny(clippy::pedantic, clippy::nursery)]

caiaphas::grammar!(
    /// Numbers in nested lists.
    pub mod list = "tests/grammars/list.ebnf"
);
caiaphas::grammar!(
    /// Words, where a name can leave what the rest needs.
    pub mod phrase = "tests/grammars/phrase.ebnf"
);
caiaphas::grammar!(
    /// Rules that take each way as it comes.
    pub mod greedy = "tests/grammars/greedy.ebnf"
);
caiaphas::grammar!(
    /// Sums of numbers in groups.
    pub mod grouped {
        "sum = group, {'+', group};
         (* inline *) group = '(', number, ')' | number;
         number = digit, {digit};
         digit = ? U+0030..U+0039 ?;"
    }
);
caiaphas::grammar!(
    /// Pairs in a private module.
    mod private { "pair = item, '=', item; item = 'x' | 'y';" }
);

#[test]
fn builds_under_strict_lints() {
    assert!(list::Numbers::parse("1,(2)").is_ok());
    assert!(phrase::Phrase::parse("affy").is_ok());
    assert!(greedy::Sentence::parse("iff;").is_ok());
    assert!(grouped::Sum::parse("(1)+2").is_ok());
    assert!(private::Pair::parse("x=y").is_ok());
}
