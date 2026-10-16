# Changelog

All notable changes to this crate are documented here, and breaking changes are
marked **Breaking**.

The format is based on [Keep a Changelog][keep-a-changelog], and this crate
adheres to [Semantic Versioning][semver].

## [Unreleased]

### Added

- The `grammar!` macro, which generates a module with a parser for an
  ISO/IEC 14977 EBNF grammar and a type for each of its rules.
- Checks for problems in a grammar, each with a suggestion, which stop the
  build unless an `(* expect code: reason *)` comment marks them as intended.
- `(* hide *)`, which leaves a rule out of the generated types.
- `(* inline *)`, which gives the rules that hold a rule what it holds instead
  of its type.
- Fields of `[T; N]` for a rule held the same number of times on every way,
  and `char()` on the type of a rule that only ever matches one character.
- Parsers that match exactly the inputs a grammar describes. Where an input
  reads more than one way, each part takes the most text it can while the
  rest still matches.
- `(* first match *)`, which makes the choices of a rule take the first
  alternative after which the rest still matches instead of the longest.
- `(* greedy *)`, which makes a rule take each way as it comes and never go
  back, so its parser stays fast.
- `Span`, which tells where a rule matched in its input, and `Error`, which
  tells where an input stops matching and what could have come there.
  `Error::expected` and `Error::found` give the same as values, with
  `Expected` at the root of the crate.
- `FromStr` for the type of each start rule.
[keep-a-changelog]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html
