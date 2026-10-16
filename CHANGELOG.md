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
- Checks for problems in a grammar, each with a code and a way to fix it.
  Errors and warnings stop the build, unless an `(* expect code: reason *)`
  comment marks a warning as intended, and suggestions stop it with `suggest`.
- `optimize` reads each start that alternatives share once, without changing
  the types.
- Annotations:
  - `(* hide *)` leaves a rule out of the generated types.
  - `(* inline *)` gives the rules that hold a rule what it holds instead of
    its type.
  - `(* as T *)` makes the type of a rule a Rust type read from its text.
  - `(* first match *)` makes the choices of a rule take the first alternative
    after which the rest still matches, instead of the longest.
  - `(* greedy *)` makes a rule take each way as it comes and never go back, so
    its parser stays fast.
- Fields of `[T; N]` for a rule held the same number of times on every way, and
  `char()` on the type of a rule that only ever matches one character.
- `Display` that writes the text of every type with `text()`.
- `FromStr` for the type of each start rule.
- `Span` for where a rule matched in its input, and `Error` for where an input
  stops matching and what could have come there. `Error::expected` and
  `Error::found` give the same as values, with `Expected` at the root of the
  crate.
- The `caiaphas` command of the `caiaphas-cli` crate. `caiaphas check` checks
  grammar files without building anything, and `caiaphas explain` tells what a
  finding means, or lists every code.
- A tutorial in the docs, a page that explains every finding, and examples that
  read arithmetic, JSON and INI files.

[keep-a-changelog]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html
