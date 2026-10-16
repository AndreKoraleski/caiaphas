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
`SyntaxRule`. The type of a start rule also has `parse`, and implements
`FromStr`, so `input.parse::<Json>()` reads it too.

* A rule whose alternatives are each another rule or a word, such as `value`
  above, becomes an enum with a variant for each, such as `Value::Object` and
  `Value::True`.
* Any other rule becomes a struct with a field for each rule it holds, named
  after that rule. A field holds `T`, `Option<T>` or `Vec<T>`, as many times as
  the rule can hold it, or `[T; N]` when it always holds it `N` times.
* A rule that only ever matches one character also has `char()`.
* Every type has `text()`, the text its rule matched, and every struct has
  `span()`, where it matched. Types own what they hold, so they need no
  lifetimes.
* A rule marked `(* hide *)`, such as `ws`, gets no type and no field.
* A rule marked `(* inline *)` gets no type, and each rule that holds it holds
  what it holds instead, as if its definition were written there.
* A rule marked `(* as T *)`, such as `(* as u16 *)`, has the Rust type `T`
  instead, read from its text with `FromStr`, and text `T` does not read fails
  to match. `T` needs `Debug`, `Clone` and `PartialEq` too.

## Grammars

caiaphas reads the syntax of ISO/IEC 14977, and works on characters, so a
grammar describes its whitespace too. A special sequence holds one code point,
such as `? U+00E9 ?`, or a range of them, such as `? U+0041..U+005A ?`.

A parser matches exactly the inputs its grammar describes. Where an input can
be read more than one way, each part, from the outside in and left to right,
takes the most text it can while the rest still matches, and the first written
of alternatives that tie wins.

The choices of a rule marked `(* first match *)` take the first alternative
after which the rest still matches instead, in the order written, so here `ab`
reads as `a` then `b`, where taking the most text would read one `ab`:

```ebnf
list = item, {item};
(* first match *)
item = 'a' | 'ab' | 'b';
```

A rule marked `(* greedy *)` takes each way as it comes and never goes back,
which keeps its parser fast where that is what the grammar means, such as the
`else` of a nested `if`.

## Problems

Before generating, caiaphas looks for what would make the parser fail, or not
do what the grammar seems to mean, such as left recursion, alternatives that
can match the same text, or a rule that can never finish matching. Each
problem stops the build with a suggestion. When a warning
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

## Acknowledgements

The scaffolding of this repository was generated with
[rust-lib-template](https://github.com/AndreKoraleski/rust-lib-template).

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
