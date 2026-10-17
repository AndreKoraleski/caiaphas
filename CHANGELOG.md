# Changelog

All notable changes to caiaphas and its crates are documented here, and
breaking changes are marked **Breaking**.

The format is based on [Keep a Changelog][keep-a-changelog], and the crates
adhere to [Semantic Versioning][semver].

## [Unreleased]

### Added

- `grammar!` generates a module with a parser for an ISO/IEC 14977 EBNF
  grammar and a type for each of its rules. The grammar comes from a file or
  from a string written in the macro.
- Parsers that match exactly the inputs a grammar describes. Where an input
  reads more than one way, each part takes the most text it can while the rest
  still matches.
- Text that must not hold a short part, written as an exception such as
  `{c} - ({c}, '*)', {c})` for the text of a comment, read in one pass.
- Checks for problems in a grammar, each with a code and a way to fix it.
  Errors and warnings stop the build, unless an `(* expect code: reason *)`
  comment marks a warning as intended, and suggestions stop it with `suggest`.
- Special sequences that hold a code point, a range of them, or a regular
  expression between two `/`, such as `? /[a-z_]+/ ?`.
- `? not /[a-z]/ ?`, which matches nothing where what it holds does not match
  what comes next, and `? end ?`, which matches nothing where the input ends.
- Alternatives that start alike, as in `a, x | a, y`, read that start once,
  as `a, (x | y)` does, without changing the types.
- Annotations, each a comment right before a rule that holds exactly it; any
  other comment is text, and a lone word written almost like one is warned of:
  - `(* hide *)` leaves a rule out of the generated types.
  - `(* inline *)` gives the rules that hold a rule what it holds instead of
    its type, under its name when that is one rule, so that rule's field
    takes the inline rule's name.
  - `(* as T *)` makes the type of a rule a Rust type read from its text.
  - `(* greedy *)` makes a rule take each way as it comes and never go back, so
    its parser stays fast.
- `Spanned<T>`, which `(* as caiaphas::Spanned<T> *)` gives: a value with where
  it was read, that otherwise reads as the `T` it holds. An enum has `span()`
  when each of its variants has one.
- Fields of `[T; N]` for a rule held the same number of times on every way, and
  `char()` on the type of a rule that only ever matches one character.
- Docs on each type that quote its rule, and begin with the comments right
  before it.
- `Display` that writes the text of every type with `text()`, and `PartialEq`,
  `Eq` and `Hash` that compare and hash by it.
- `parse` and `FromStr` on the type of every rule, so a part of a grammar can
  be read and tested alone, and `parse_at` for an input that starts partway
  through a larger text, so its errors and spans count lines from there.
- A limit on how deeply the matches of recursive rules nest, so that no input
  can make a parser run out of stack. A deeper input gives an error, and
  `Error::is_too_deep` tells which. `depth = N` in `grammar!` changes the
  limit.
- `(* recover *)` and `parse_recovering` on each type, which go on past each
  error that a rule written with it can skip, and give every error found with
  the value of what still matches, as `Recovered`.
- `Span` for where a rule matched in its input, and `Error` for where an input
  stops matching and what could have come there. `Error::expected` and
  `Error::found` give the same as values, with `Expected` at the root of the
  crate. `Error::excerpt` gives the line of the error with a caret under it,
  and `Span::excerpt` the line of a match with carets under its text.
- The `caiaphas` command of the `caiaphas-cli` crate. `caiaphas check` checks
  grammar files without building anything, `caiaphas types` prints the types
  a grammar gives, `caiaphas parse` runs a grammar on inputs as its parser
  would and prints the tree or every error, or with `--expect-fail` checks
  that none matches, and `caiaphas explain` tells what a finding means, or
  lists every code.
- A tutorial in the docs, a page that explains every finding, and examples that
  read arithmetic, JSON, INI and CSV files, and logs.

[keep-a-changelog]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html
