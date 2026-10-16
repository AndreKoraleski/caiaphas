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
//! `setting` is the start rule, since no other rule uses it, so its type
//! parses: with `parse`, or `str::parse` as here. A comment right before a
//! rule can tell caiaphas more about it: `(* hide *)` leaves `letter` and
//! `digit` out of the types, and `(* as u16 *)` reads `port` as a `u16`.
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
//! | `(* note *)`      | nothing, as a comment                        |
//!
//! ISO 14977 leaves the meaning of a special sequence, written between `?`, to
//! each tool. caiaphas gives it one: a code point, or a range of them. It is
//! the supported way to write a character that no terminal string holds, such
//! as `é` or a tab.
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
//! - A rule whose alternatives are each a rule or a word becomes an enum.
//! - Any other rule becomes a struct. A field holds `T`, `Option<T>`, `Vec<T>`
//!   or `[T; N]`, as many times as the rule can hold it, and a type that could
//!   contain itself holds itself in a `Box`.
//! - Every struct has `text()` and `span()`, and a rule that only ever matches
//!   one character also has `char()`.
//! - `(* hide *)` gives a rule no type and no field, and `(* inline *)` gives
//!   it no type, with each rule that holds it holding what it holds instead.
//! - `(* as T *)` makes a rule's type the Rust type `T`, read from its text
//!   with `FromStr`. Text `T` does not read fails to match.
//!
//! # Errors
//!
//! An input that does not match tells where it stops matching, and what could
//! have come there:
//!
//! ```
//! # caiaphas::grammar!(pub mod setting {
//! #     "setting = name, '=', port;
//! #      (* as String *) name = letter, {letter};
//! #      (* as u16 *) port = digit, {digit};
//! #      (* hide *) letter = ? U+0061..U+007A ?;
//! #      (* hide *) digit = ? U+0030..U+0039 ?;"
//! # });
//! let error = "web=99999".parse::<setting::Setting>().unwrap_err();
//! assert_eq!(
//!     error.to_string(),
//!     "expected a port, found `99999` at line 1, column 5"
//! );
//! assert_eq!((error.line(), error.column()), (1, 5));
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
//! rules where that is not so, and only those try every way. Two annotations
//! change how a rule chooses:
//!
//! - `(* first match *)` makes its choices take the first alternative after
//!   which the rest still matches, in the order written.
//! - `(* greedy *)` makes it take each way as it comes and never go back, as
//!   the `else` of a nested `if` does. That keeps its parser fast.
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
//! module list them, and `grammar!(..., optimize)` rewrites what it safely
//! can, such as a start that alternatives share, without changing the types.
//!
//! The `caiaphas` command of the `caiaphas-cli` crate checks grammar files
//! without building anything, and tells what each finding means:
//!
//! ```text
//! caiaphas check grammars/json.ebnf
//! caiaphas explain tied-branches
//! ```
