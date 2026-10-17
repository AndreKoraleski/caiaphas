//! A tour of caiaphas, from a first grammar to the checks it runs on one.
//!
//! # A first grammar
//!
//! A grammar is a list of rules in ISO/IEC 14977 EBNF. `grammar!` turns one
//! into a module with a parser and a type for each rule:
//!
//! ```
//! caiaphas::grammar!(pub mod setting {
//!     "setting = name, '=', port;
//!      (* as String *) name = letter, {letter};
//!      (* as u16 *) port = digit, {digit};
//!      (* hide *) letter = ? U+0061..U+007A ?;
//!      (* hide *) digit = ? U+0030..U+0039 ?;"
//! });
//!
//! fn main() -> Result<(), caiaphas::Error> {
//!     let setting: setting::Setting = "web=8080".parse()?;
//!     assert_eq!(setting.name, "web");
//!     assert_eq!(setting.port, 8080);
//!     Ok(())
//! }
//! ```
//!
//! The type of every rule parses: with `parse`, or `str::parse` as here. The
//! comments right before some rules are annotations, and [Types](#types) tells
//! what they do.
//!
//! A longer grammar goes in a file, with a path from the root of the crate:
//!
//! ```text
//! caiaphas::grammar!(pub mod json = "grammars/json.ebnf");
//! ```
//!
//! # Grammars
//!
//! A grammar works on characters, so it describes its whitespace too. Each
//! rule ends with `;` or `.`:
//!
//! | Written           | Matches                                      |
//! |-------------------|----------------------------------------------|
//! | `'if'` or `"if"`  | that text                                    |
//! | `a, b`            | `a`, then `b`                                |
//! | `a \| b`          | `a` or `b`                                   |
//! | `[a]`             | `a`, or nothing                              |
//! | `{a}`             | `a` any number of times, or nothing          |
//! | `(a \| b), c`     | a group, as in arithmetic                    |
//! | `3 * a`           | `a` three times                              |
//! | `a - b`           | what `a` matches but `b` does not            |
//! | `? U+00E9 ?`      | the character `é`                            |
//! | `? U+0061..U+007A ?` | any character from `a` to `z`             |
//! | `? /[a-z_]+/ ?`   | what the regular expression matches          |
//! | `(* note *)`      | nothing, as a comment                        |
//!
//! ISO 14977 leaves the meaning of a special sequence, written between `?`, to
//! each tool. caiaphas gives it one: a code point, a range of them, or a
//! regular expression between two `/`. It is the supported way to write a
//! character that no terminal string holds, such as `é` or a tab.
//!
//! A regular expression holds characters, classes such as `[a-z_]`, `[^"]`,
//! `.`, `\d`, `\w` and `\s`, choices with `|`, groups, and repetitions with
//! `*`, `+`, `{n}`, `{n,}` and `{n,m}`. `U+XXXX` stands for a code point, and
//! `{0,1}` for an optional part, since a `?` would end the special sequence.
//! It matches as the grammar it stands for would:
//!
//! ```
//! caiaphas::grammar!(mod assignment {
//!     r"assignment = name, ' = ', number;
//!       name = ? /[a-z_][a-z0-9_]*/ ?;
//!       number = ? /\d+(\.\d+){0,1}/ ?;"
//! });
//!
//! let parsed = assignment::Assignment::parse("pi = 3.14").unwrap();
//! assert_eq!((parsed.name.text(), parsed.number.text()), ("pi", "3.14"));
//! ```
//!
//! # Types
//!
//! Each rule becomes a type named after it, and fields are named after the
//! rules they hold:
//!
//! ```
//! caiaphas::grammar!(mod shape {
//!     "shape = circle | square;
//!      circle = 'circle ', size;
//!      square = 'square ', size, 'x', size;
//!      (* as u32 *) size = digit, {digit};
//!      (* hide *) digit = ? U+0030..U+0039 ?;"
//! });
//!
//! use shape::Shape;
//!
//! let area = match Shape::parse("square 3x4").unwrap() {
//!     Shape::Circle(circle) => 3 * circle.size * circle.size,
//!     Shape::Square(square) => {
//!         let [width, height] = square.size;
//!         width * height
//!     }
//! };
//! assert_eq!(area, 12);
//! ```
//!
//! - A rule whose alternatives are each a rule or a word becomes an enum. An
//!   inline rule that holds one rule once, as in `group = '(', sum, ')'`, can
//!   be an alternative too, and gives the variant `Group(Sum)`. A group of
//!   rules among parts that hold nothing, as in `(a | b), ws` with `ws`
//!   hidden, becomes an enum too.
//! - Any other rule becomes a struct. A field holds `T`, `Option<T>`, `Vec<T>`
//!   or `[T; N]`, as many times as the rule can hold it, and a type that could
//!   contain itself holds itself in a `Box`.
//! - A struct has one field for each rule it holds, wherever it holds it, so
//!   in `sum = term, {'+', term};` every `term` is in one `Vec<Term>`. A
//!   repeated group with a rule of its own keeps each of its matches together:
//!   `sum = term, {addition}; addition = '+', term;`.
//! - Every struct has `text()` and `span()`, and a rule that only ever matches
//!   one character also has `char()`. A type with `text()` displays as that
//!   text.
//! - A comment right before a rule, with no empty line between, begins the
//!   docs of its type, unless it is an annotation.
//! - `(* hide *)` gives a rule no type and no field, and `(* inline *)` gives
//!   it no type, with each rule that holds it holding what it holds instead.
//! - `(* as T *)` makes a rule's type the Rust type `T`, read from its text
//!   with `FromStr`. `T` is named as where `grammar!` is called, so it can be
//!   a type of your own, generic or not. Text `T` does not read fails to
//!   match, so a rule with `(* hide *)` too keeps no value but still checks
//!   its text.
//!
//! # Errors
//!
//! An input that does not match tells where it stops matching, and what could
//! have come there. `excerpt` gives the line of the error with a caret under
//! where it is:
//!
//! ```
//! # caiaphas::grammar!(pub mod setting {
//! #     "setting = name, '=', port;
//! #      (* as String *) name = letter, {letter};
//! #      (* as u16 *) port = digit, {digit};
//! #      (* hide *) letter = ? U+0061..U+007A ?;
//! #      (* hide *) digit = ? U+0030..U+0039 ?;"
//! # });
//! let input = "web=99999";
//! let error = input.parse::<setting::Setting>().unwrap_err();
//! assert_eq!(
//!     error.to_string(),
//!     "expected port, found `99999` at line 1, column 5"
//! );
//! assert_eq!((error.line(), error.column()), (1, 5));
//! assert_eq!(error.excerpt(input), "1 | web=99999\n  |     ^");
//! ```
//!
//! A span has `excerpt` too, for a program's own message about a match, such
//! as a port it does not allow:
//!
//! ```
//! # caiaphas::grammar!(pub mod setting {
//! #     "setting = name, '=', port;
//! #      (* as String *) name = letter, {letter};
//! #      port = digit, {digit};
//! #      (* hide *) letter = ? U+0061..U+007A ?;
//! #      (* hide *) digit = ? U+0030..U+0039 ?;"
//! # });
//! let setting: setting::Setting = "web=0".parse().unwrap();
//! assert_eq!(setting.port.span().excerpt(), "1 | web=0\n  |     ^");
//! ```
//!
//! A rule written with `(* recover *)` lets a parse go on past an error in it.
//! Where a match of the rule fails partway, `parse_recovering` skips to its
//! next match from the error on, and gives every error found, with the value
//! of what still matches:
//!
//! ```
//! caiaphas::grammar!(mod settings {
//!     r"settings = [line], {newline, [line]};
//!       (* recover *)
//!       line = name, ' = ', value;
//!       name = ? /[a-z]+/ ?;
//!       value = ? /[^\n]+/ ?;
//!       (* hide *) newline = ? U+000A ?;"
//! });
//!
//! let input = "a=1\nb = 2\nc = 3";
//! let recovered = settings::Settings::parse_recovering(input);
//! assert_eq!(recovered.value.unwrap().line.len(), 2);
//! assert_eq!(
//!     recovered.errors[0].to_string(),
//!     "expected ` = ` or a lowercase letter, found `=` at line 1, column 2"
//! );
//! ```
//!
//! # Matching
//!
//! A grammar can describe the same input in more than one way. In a file name
//! such as `archive.tar.gz`, a name can hold dots, so the grammar alone does
//! not say where the name ends:
//!
//! ```
//! caiaphas::grammar!(mod file {
//!     "file = name, '.', extension;
//!      (* as String *) name = character, {character};
//!      (* as String *) extension = letter, {letter};
//!      (* hide *) character = letter | '.';
//!      (* hide *) letter = ? U+0061..U+007A ?;"
//! });
//!
//! let file = file::File::parse("archive.tar.gz").unwrap();
//! assert_eq!(file.name, "archive.tar");
//! assert_eq!(file.extension, "gz");
//! ```
//!
//! A parser matches every input its grammar describes. Where there is more
//! than one way, each part takes the most text it can while the rest still
//! matches, so the name takes `archive.tar` and leaves `.gz`. Of alternatives
//! that match the same text, the first written wins. A parser that took each
//! way as it came would have read the whole input as the name, and then found
//! no `.` for the extension.
//!
//! Most choices need no such care, since the next character tells which way
//! goes on, and the parser takes those as it comes. caiaphas finds the few
//! rules where that is not so, and only those try every way. `(* greedy *)`
//! makes a rule take each way as it comes and never go back, as the `else` of
//! a nested `if` does. That keeps its parser fast.
//!
//! Text that must not hold something, such as the text of a comment, reads
//! well as an exception. Where what it must not hold is short, the parser
//! reads such text in one pass, up to the first place that holds it:
//!
//! ```
//! caiaphas::grammar!(mod comment {
//!     "comment = '(*', text, '*)';
//!      (* as String *) text = {character} - ({character}, '*)', {character});
//!      (* hide *) character = ? /./ ?;"
//! });
//!
//! let comment = comment::Comment::parse("(* a (b) *)").unwrap();
//! assert_eq!(comment.text, " a (b) ");
//! assert!(comment::Comment::parse("(* a *) b *)").is_err());
//! ```
//!
//! # Problems and suggestions
//!
//! Before generating, caiaphas checks the grammar. An error, such as left
//! recursion, stops the build. So does a warning, such as two alternatives
//! that can match the same text, unless a comment right before its rule
//! expects it, with its code and a reason:
//!
//! ```text
//! (* expect tied-branches: a keyword is never a name *)
//! word = keyword | name;
//! ```
//!
//! A suggestion, such as where a rule has to try every way, does not stop the
//! build unless `grammar!(..., suggest)` asks it to. The docs of the generated
//! module list them.
//!
//! The [findings] page explains every finding. The `caiaphas` command of the
//! `caiaphas-cli` crate checks grammar files without building anything, and
//! tells what each finding means:
//!
//! ```text
//! caiaphas check grammars/json.ebnf
//! caiaphas explain tied-branches
//! ```
//!
//! [findings]: https://docs.rs/caiaphas-generator/latest/caiaphas_generator/findings/
