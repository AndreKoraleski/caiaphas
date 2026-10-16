# caiaphas

[![CI][ci-badge]][ci]
[![crates.io][crates-badge]][crates]
[![docs.rs][docs-badge]][docs]

EBNF parser generator for Rust that detects grammar ambiguities and suggests
fixes

1. [Introduction](#introduction)
2. [Tutorial](#tutorial)
3. [Overview](#overview)
4. [Grammar syntax](#grammar-syntax)
5. [Types](#types)
6. [Annotations](#annotations)
7. [Matching](#matching)
8. [Checks](#checks)
9. [Errors](#errors)
10. [Macro](#macro)
11. [Command line](#command-line)
12. [Examples](#examples)
13. [Limitations](#limitations)

## Introduction

caiaphas generates parsers from grammars written in the EBNF that ISO/IEC 14977
defines. Its `grammar!` macro reads a grammar when your crate compiles, and
writes a module with a parser and a Rust type for each rule. The types own what
they hold, so there are no lifetimes to carry around, and they follow the
rules, so the shape of the grammar is the shape of the data.

A parser matches exactly the inputs its grammar describes. Before writing
anything, caiaphas checks the grammar, and stops with a way to fix each
problem it finds, such as left recursion or two alternatives that can match
the same text.

```sh
cargo add caiaphas
```

## Tutorial

The [tutorial] goes from a first grammar to the checks caiaphas runs, and its
examples run as tests. The [examples](#examples) are whole programs.

## Overview

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

Then generate its module, with a path from the root of your crate, and parse
with the type of the rule that no other rule uses:

```rust
caiaphas::grammar!(pub mod json = "grammars/json.ebnf");

use json::{Json, Value};

let document: Json = r#"{"name": "caiaphas"}"#.parse()?;
if let Value::Object(object) = &document.value {
    for member in &object.member {
        println!("{} is {}", member.string.text(), member.value.text());
    }
}
```

## Grammar syntax

A grammar works on characters, so it describes its whitespace too. Each rule
ends with `;` or `.`.

| Written              | Matches                                 |
|----------------------|-----------------------------------------|
| `'if'` or `"if"`     | that text                               |
| `a, b`               | `a`, then `b`                           |
| `a \| b`             | `a` or `b`                              |
| `[a]`                | `a`, or nothing                         |
| `{a}`                | `a` any number of times, or nothing     |
| `(a \| b), c`        | a group, as in arithmetic               |
| `3 * a`              | `a` three times                         |
| `a - b`              | what `a` matches but `b` does not       |
| `? U+00E9 ?`         | the character `é`                       |
| `? U+0061..U+007A ?` | any character from `a` to `z`           |
| `(* note *)`         | nothing, as a comment                   |

ISO 14977 leaves the meaning of a special sequence, written between `?`, to
each tool. caiaphas gives it one: a code point, or a range of them. It is the
supported way to write a character that no terminal string holds, such as `é`
or a tab.

## Types

Each rule becomes a type named after it, and each field is named after the
rule it holds. This grammar:

```ebnf
shape = circle | square;
circle = 'circle ', size;
square = 'square ', size, 'x', size;
(* as u32 *) size = digit, {digit};
(* hide *) digit = ? U+0030..U+0039 ?;
```

gives these types, in short:

```rust
pub enum Shape { Circle(Circle), Square(Square) }
pub struct Circle { pub size: u32 }
pub struct Square { pub size: [u32; 2] }
```

* A rule whose alternatives are each another rule or a word becomes an enum,
  and any other rule a struct.
* A field holds `T`, `Option<T>` for `[a]`, `Vec<T>` for `{a}`, or `[T; N]`
  when the rule always holds it `N` times. A type that could contain itself
  holds itself in a `Box`.
* Every struct gives the text it matched with `text()`, and where with
  `span()`. A rule that only ever matches one character also has `char()`.
* The type of a start rule has `parse` and implements `FromStr`. A start rule
  is one that no other rule uses.

## Annotations

A comment right before a rule can tell caiaphas more about it.

| Annotation                  | Effect                                         |
|-----------------------------|------------------------------------------------|
| `(* hide *)`                | no type and no field, as for whitespace        |
| `(* inline *)`              | no type: rules that hold it hold what it holds |
| `(* as u16 *)`              | the type is `u16`, read from the rule's text   |
| `(* first match *)`         | choices take the first alternative that works  |
| `(* greedy *)`              | the rule takes each way as it comes            |
| `(* expect code: reason *)` | marks a warning as intended                    |

`(* as T *)` takes any type with `FromStr`, `Debug`, `Clone` and `PartialEq`,
such as `String`, `f64` or `std::net::Ipv4Addr`. Text the type does not read
fails to match there, as `300` does for a `u8`.

## Matching

A grammar can describe the same input in more than one way. In a file name
such as `archive.tar.gz`, a name can hold dots, so the grammar alone does not
say where the name ends:

```ebnf
file = name, '.', extension;
name = character, {character};
extension = letter, {letter};
character = letter | '.';
```

A parser matches every input its grammar describes. Where there is more than
one way, each part takes the most text it can while the rest still matches, so
the name takes `archive.tar` and leaves `.gz`. Of alternatives that match the
same text, the first written wins.

Most choices need no such care, since the next character tells which way goes
on, and the parser takes those as it comes. caiaphas finds the few rules where
that is not so, and only those try every way. `(* first match *)` makes a
rule's choices take the first alternative after which the rest still matches,
and `(* greedy *)` makes a rule take each way as it comes and never go back, as
the `else` of a nested `if` does.

## Checks

Before generating, caiaphas checks the grammar. Each finding has a code,
and `caiaphas explain <code>` tells what it means, with an example and the
usual fix.

* An error stops the build, as `left-recursion` and `unproductive-rule` do.
* A warning stops it too, as `tied-branches` and `unreachable-alternative` do,
  unless a comment right before its rule expects it with a reason:

  ```ebnf
  (* expect tied-branches: a keyword is never an identifier *)
  token = keyword | identifier;
  ```

* A suggestion, such as `nondeterministic-choice`, does not stop the build
  unless the `suggest` flag asks it to. The docs of the generated module list
  them.

The [findings] page lists every code.

## Errors

An input that does not match tells where it stops matching, and what could
have come there:

```text
expected `,`, `}`, a digit, an exponent or a fraction, found `]` at line 1, column 8
```

`Error` displays as that sentence, and `line()`, `column()`, `offset()`,
`expected()` and `found()` give the same as values.

## Macro

```rust
// A grammar in a file, with a path from the root of the crate.
caiaphas::grammar!(pub mod json = "grammars/json.ebnf");

// A short grammar written in the macro.
caiaphas::grammar!(mod pair { "pair = 'x', ',', 'y';" });

// Stop at every suggestion too.
caiaphas::grammar!(pub mod json = "grammars/json.ebnf", suggest);

// Rewrite what can be, such as a start that alternatives share, without
// changing the types.
caiaphas::grammar!(pub mod json = "grammars/json.ebnf", optimize);
```

Attributes and doc comments before `mod` go to the generated module, and Cargo
builds again when the grammar's file changes.

## Command line

The `caiaphas` command checks grammar files without building anything, and
prints every finding, suggestions included. It exits with 1 when one would
stop the build, so it suits a CI step.

```sh
cargo install caiaphas-cli
caiaphas check grammars/json.ebnf
caiaphas explain tied-branches
```

## Examples

| Example                              | Shows                                       |
|--------------------------------------|---------------------------------------------|
| [calculator](examples/calculator.rs) | enums, repetitions, recursion and `f64`s    |
| [json](examples/json.rs)             | a whole format, read from standard input    |
| [ini](examples/ini.rs)               | lines, hidden rules and `(* as String *)`   |

```sh
cargo run --example calculator -- "1.5 + 2 * (3 - 4)"
```

## Limitations

* Left recursion is an error. Its message tells how to write the rule as a
  repetition.
* There is no separate lexer, so a grammar describes its whitespace.
* A parser reads the whole input as a `&str`, and stops at the first place it
  cannot match.

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
[tutorial]: https://docs.rs/caiaphas/latest/caiaphas/tutorial/
[findings]: https://docs.rs/caiaphas-generator/latest/caiaphas_generator/findings/
