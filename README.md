# caiaphas

[![CI][ci-badge]][ci]
[![crates.io][crates-badge]][crates]
[![docs.rs][docs-badge]][docs]

EBNF parser generator for Rust that detects grammar ambiguities and suggests
fixes

Write a grammar in ISO/IEC 14977 EBNF, and caiaphas turns it into a parser
when your crate compiles, with a Rust type for each rule. It checks the grammar
first, and stops with a suggestion for every problem it finds.

## Installation

```sh
cargo add caiaphas
```

## Usage

Write the grammar in a file, such as `grammars/json.ebnf`:

```ebnf
json = ws, value, ws;
value = object | array | string | number | 'true' | 'false' | 'null';
object = '{', ws, [member, ws, {',', ws, member, ws}], '}';
member = string, ws, ':', ws, value;
(* ... *)
(* hide *)
ws = {' ' | ? U+0009 ? | ? U+000A ? | ? U+000D ?};
```

Then generate its module, with a path relative to the root of your crate, and
parse with the type of a rule that no other rule uses:

```rust
caiaphas::grammar!(pub mod json = "grammars/json.ebnf");

use json::{Json, Value};

let document = Json::parse(r#"{"name": "caiaphas"}"#)?;
if let Value::Object(object) = &document.value {
    for member in &object.member {
        println!("{} is {}", member.string.text(), member.value.text());
    }
}
```

An input that does not match says why, such as ``expected `,`, `}`, a digit,
an exponent or a fraction, found `]` at line 1, column 8``.

## Types

Each rule becomes a type named after it, so `syntax rule` becomes
`SyntaxRule`.

* A rule whose alternatives are each another rule or a word, such as `value`
  above, becomes an enum with a variant for each, such as `Value::Object` and
  `Value::True`.
* Any other rule becomes a struct with a field for each rule it holds, named
  after that rule. A field holds `T`, `Option<T>` or `Vec<T>`, as many times as
  the rule can hold it.
* Every type has `text()`, the text its rule matched, and every struct has
  `span()`, where it matched. Types own what they hold, so they need no
  lifetimes.
* A rule marked `(* hide *)`, such as `ws`, gets no type and no field.

## Grammars

caiaphas reads the syntax of ISO/IEC 14977, and works on characters, so a
grammar describes its whitespace too. A special sequence holds one code point,
such as `? U+00E9 ?`, or a range of them, such as `? U+0041..U+005A ?`.

Where a choice has more than one way to go, a parser takes the alternative
that matches the most text, and optional and repeated sequences take as much
as they can.

## Problems

Before generating, caiaphas looks for what would make the parser fail, or not
do what the grammar seems to mean, such as left recursion, alternatives that
can match the same text, or a repetition that leaves nothing for what comes
after it. Each problem stops the build with a suggestion. When a warning
describes what you intend, mark it right before its rule with the code the
suggestion gives and a reason:

```ebnf
(* expect tied-branches: a keyword is never an identifier *)
token = keyword | identifier;
```

## Minimum supported Rust version

Rust 1.85 or later.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

[ci-badge]: https://github.com/AndreKoraleski/caiaphas/actions/workflows/ci.yml/badge.svg
[ci]: https://github.com/AndreKoraleski/caiaphas/actions/workflows/ci.yml
[crates-badge]: https://img.shields.io/crates/v/caiaphas.svg
[crates]: https://crates.io/crates/caiaphas
[docs-badge]: https://img.shields.io/docsrs/caiaphas
[docs]: https://docs.rs/caiaphas
