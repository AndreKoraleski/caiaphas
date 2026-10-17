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
they hold, so they carry no lifetimes, and each follows its rule, so the data
has the shape of the grammar.

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
with the type of its [start rule](#types), `json`:

```rust
caiaphas::grammar!(pub mod json = "grammars/json.ebnf");

use json::{Json, Value};

let document: Json = r#"{"name": "caiaphas"}"#.parse()?;
if let Value::Object(object) = &document.value {
    for member in &object.member {
        if let Value::String(value) = &member.value {
            println!("{} is {}", member.string, value);
        }
    }
}
```

## Grammar syntax

Each rule ends with `;` or `.`, and whitespace is text like any other. A hidden
rule, such as `ws` in the [overview](#overview), keeps it out of the types.

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
| `? /[a-z_]+/ ?`      | what the regular expression matches     |
| `(* note *)`         | nothing, as a comment                   |

The standard's other spellings work too: `/` or `!` for `|`, `(/ /)` for
`[ ]` and `(: :)` for `{ }`.

ISO 14977 leaves the meaning of a special sequence, written between `?`, to
each tool. caiaphas gives it one: a code point, a range of them, or a regular
expression between two `/`. It is the supported way to write a character that
no terminal string holds, such as `é` or a tab.

A regular expression holds characters, classes such as `[a-z_]`, `[^"]`, `.`,
`\d`, `\w` and `\s`, choices with `|`, groups, and repetitions with `*`, `+`,
`{n}`, `{n,}` and `{n,m}`. `U+XXXX` stands for a code point, and `{0,1}` for
an optional part, since a `?` would end the special sequence. It matches as
the grammar it stands for would:

```ebnf
assignment = name, ' = ', number;
name = ? /[a-z_][a-z0-9_]*/ ?;
number = ? /\d+(\.\d+){0,1}/ ?;
```

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

* A rule whose alternatives are each another rule or a word, such as
  `'true'`, becomes an enum, and any other rule a struct. An alternative can
  also be an inline rule that holds one rule once, so with
  `(* inline *) group = '(', sum, ')';` it gives the variant `Group(Sum)`. A
  group of rules among parts that hold nothing, such as `(a | b), ws` with
  `ws` hidden, becomes an enum too.
* A type is named in upper camel case and a field in snake case, so
  `syntax rule` gives `SyntaxRule` and `syntax_rule`. A word Rust keeps, such
  as `type`, becomes `r#type`, and `self` and `span` become `self_` and
  `span_`.
* A field holds `T`, `Option<T>` for `[a]`, `Vec<T>` for `{a}`, or `[T; N]`
  when the rule always holds it `N` times. A type that could contain itself
  holds itself in a `Box`.
* Every struct gives the text it matched with `text()`, and where with
  `span()`. An enum has `text()` too, unless a variant holds an `(* as T *)`
  value. A type with `text()` displays as that text, and a rule that only
  ever matches one character also has `char()`.
* A comment right before a rule, with no empty line between, begins the docs
  of its type, unless it is an [annotation](#annotations). The docs then
  quote the rule.
* Two matches are equal, and hash alike, when they matched the same text,
  wherever they matched it.
* Every type has `parse` and implements `FromStr`, so a part of a grammar can
  be read and tested alone. A start rule is one that no other rule uses, or
  the first rule when every rule is used, as when the top rule recurs.

## Annotations

A comment right before a rule can tell caiaphas more about it.

| Annotation                  | Effect                                         |
|-----------------------------|------------------------------------------------|
| `(* hide *)`                | no type and no field, as for whitespace        |
| `(* inline *)`              | no type: rules that hold it hold what it holds |
| `(* as u16 *)`              | the type is `u16`, read from the rule's text   |
| `(* match first *)`         | how the rule chooses, see [Matching](#matching)|
| `(* greedy *)`              | how the rule chooses, see [Matching](#matching)|
| `(* expect code: reason *)` | marks a warning or suggestion as intended      |

`(* as T *)` takes any type with `FromStr`, `Debug`, `Clone` and `PartialEq`,
named as where `grammar!` is called, such as `String`, `f64`,
`std::net::Ipv4Addr` or a type of your own, generic or not. Text the type does
not read fails to match there, as `300` does for a `u8`.

With `(* hide *)` too, a rule keeps no value but still checks its text, so
`address` here holds no fields and still turns down `10.0.0.256`:

```ebnf
address = byte, '.', byte, '.', byte, '.', byte;
(* hide *) (* as u8 *) byte = ? /\d+/ ?;
```

## Matching

A grammar can describe the same input in more than one way. In a file name
such as `archive.tar.gz`, a name can hold dots, so the grammar alone does not
say where the name ends:

```ebnf
file = name, '.', extension;
name = character, {character};
extension = letter, {letter};
character = letter | '.';
letter = ? U+0061..U+007A ?;
```

Where an input reads more than one way, each part takes the most text it can
while the rest still matches, so the name takes `archive.tar` and leaves
`.gz`. Of alternatives that match the same text, the first written wins.

Most choices need no such care, since the next character tells which way goes
on, and the parser takes those as it comes. caiaphas finds the few rules where
that is not so, and only those try every way. `(* match first *)` makes a
rule's choices take the first alternative after which the rest still matches,
and `(* greedy *)` makes a rule take each way as it comes and never go back, as
the `else` of a nested `if` does.

## Checks

Each finding has a code in its title, as in `warning[tied-branches]`, and
`caiaphas explain <code>` tells what it means, with an example and the usual
fix.

* An error stops the build, as `unproductive-rule` and
  `nullable-repeated-sequence` do.
* A warning stops it too, as `tied-branches` and `unreachable-alternative` do,
  unless a comment right before its rule expects it with a reason:

  ```ebnf
  (* expect tied-branches: a keyword is never a name *)
  word = keyword | name;
  keyword = 'if' | 'else';
  name = ? /[a-z]+/ ?;
  ```

* A suggestion, such as `nondeterministic-choice`, does not stop the build
  unless the `suggest` flag asks it to. The docs of the generated module list
  them.

The [findings] page lists every code.

## Errors

An input that does not match tells where it stops matching, and what could
have come there. For `{"a": 1]`, the parser of the [overview](#overview)
gives:

```text
expected `,`, `}`, a digit, an exponent or a fraction, found `]` at line 1, column 8
```

`Error` displays as that sentence, and `line()`, `column()`, `offset()`,
`expected()` and `found()` give the same as values.

## Macro

```rust
// A grammar in a file, with a path from the root of the crate.
caiaphas::grammar!(pub mod json = "grammars/json.ebnf");

// A short grammar written in the macro, in a raw string when it holds `"`.
caiaphas::grammar!(mod pair { "pair = 'x', ',', 'y';" });
caiaphas::grammar!(mod quoted { r#"quoted = '"', 'x', '"';"# });

// Stop at every suggestion too.
caiaphas::grammar!(pub mod json = "grammars/json.ebnf", suggest);

// Read a start that alternatives share once, without changing the types.
caiaphas::grammar!(pub mod json = "grammars/json.ebnf", optimize);

// Attributes and doc comments go before `mod`, inside the macro.
caiaphas::grammar!(
    /// Reads JSON.
    pub mod json = "grammars/json.ebnf"
);
```

Cargo builds again when the grammar's file changes.

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
| [csv](examples/csv.rs)               | regular expressions and a type of your own  |
| [log](examples/log.rs)               | an enum of your own and `(* greedy *)`      |

```sh
cargo run --example calculator -- "1.5 + 2 * (3 - 4)"
```

## Limitations

* Left recursion is an error. Its message tells how to write the rule as a
  repetition.
* There is no separate lexer, so a grammar describes its whitespace.
* An exception cannot use a recursive rule after the `-`.
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
