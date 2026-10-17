//! Tests that running a grammar's plan matches as its generated parser does:
//! the same tree for each input that matches, and the same error for each that
//! does not, with or without recovering. The inputs come from the grammar
//! itself, each followed by changes that make most of them wrong.

use std::{mem, thread};

use caiaphas::Error;
use caiaphas_generator::plan::{self, Kind, Part, Plan};

use crate::interpret::Program;

/// Generates inputs from a plan, the same ones for the same seed.
struct Inputs<'p> {
    plan: &'p Plan,
    state: u64,
}

/// How long an input gets before its parts stop being written out.
const LENGTH: usize = 400;

/// How deep the rules of an input nest before its parts stop being written.
const NESTING: usize = 24;

impl Inputs<'_> {
    fn next(&mut self) -> u64 {
        // A xorshift generator is enough to pick among a few ways.
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn below(&mut self, count: usize) -> usize {
        usize::try_from(self.next() % count.max(1) as u64).unwrap_or(0)
    }

    /// Adds a text the rule numbered `rule` may match to `input`.
    fn rule(&mut self, rule: usize, depth: usize, input: &mut String) {
        if depth < NESTING && input.len() < LENGTH {
            self.part(&self.plan.rules[rule].expression, depth + 1, input);
        }
    }

    fn part(&mut self, part: &Part, depth: usize, input: &mut String) {
        match &part.kind {
            Kind::Sequence(steps) => {
                for step in steps {
                    self.part(step, depth, input);
                }
            }
            Kind::Choice(alternatives) => {
                let alternative = self.below(alternatives.len());
                self.part(&alternatives[alternative], depth, input);
            }
            Kind::Optional(sequence) => {
                if self.below(2) == 0 {
                    self.part(sequence, depth, input);
                }
            }
            Kind::Repeat(sequence) => {
                for _ in 0..self.below(4) {
                    if input.len() >= LENGTH {
                        break;
                    }
                    self.part(sequence, depth, input);
                }
            }
            Kind::Times(times, primary) => {
                for _ in 0..*times {
                    self.part(primary, depth, input);
                }
            }
            Kind::Except { factor, .. } => self.part(factor, depth, input),
            Kind::Rule(rule) => self.rule(*rule, depth, input),
            Kind::Text(text) => input.push_str(text),
            Kind::Range(first, last) => {
                let (first, last) = (u32::from(*first), u32::from(*last));
                let picked = match self.below(3) {
                    0 => first,
                    1 => last,
                    _ => first + (last - first) / 2,
                };
                input.push(char::from_u32(picked).unwrap_or('x'));
            }
            _ => {}
        }
    }

    /// Returns `input` with one change: a character left out, doubled, or put
    /// in from elsewhere in it.
    fn changed(&mut self, input: &str) -> String {
        let chars: Vec<char> = input.chars().collect();
        if chars.is_empty() {
            return "?".to_owned();
        }
        let at = self.below(chars.len());
        let mut changed = chars.clone();
        match self.below(3) {
            0 => {
                changed.remove(at);
            }
            1 => changed.insert(at, chars[at]),
            _ => changed.insert(at, chars[self.below(chars.len())]),
        }
        changed.into_iter().collect()
    }
}

/// What the parser `grammar!` generates gives for an input: the tree or the
/// error, then the tree and errors of a parse that recovers.
#[derive(Clone, Copy)]
struct Generated {
    parse: fn(&str) -> Result<String, Error>,
    recovering: fn(&str) -> (Option<String>, Vec<Error>),
}

/// Checks that the grammar `source`, run from its plan starting with the rule
/// named `start`, matches as `generated` does, on inputs from `seed` and on
/// the `written` ones.
fn compare(
    source: &'static str,
    start: &str,
    generated: Generated,
    seed: u64,
    written: &[&str],
) {
    let plan = plan::of(source, "grammar.ebnf").expect("the grammar works");
    let start = plan
        .rules
        .iter()
        .position(|rule| rule.name == start)
        .expect("the grammar has the rule");
    let mut inputs = Inputs {
        plan: &plan,
        state: seed,
    };
    let mut all = Vec::new();
    for _ in 0..60 {
        let mut input = String::new();
        inputs.rule(start, 0, &mut input);
        let changed = inputs.changed(&input);
        let changed_again = inputs.changed(&changed);
        all.extend([input, changed, changed_again]);
    }
    all.push(String::new());
    all.extend(written.iter().map(|&input| input.to_owned()));
    // Without a rule that recovers, a parse that recovers runs the same rules
    // as one that does not.
    let recovers = plan.rules.iter().any(|rule| rule.recovers);
    // An exception read in one scan of a run is checked against reading it as
    // any other exception.
    let mut general = plan.clone();
    let mut scanned = false;
    for rule in &mut general.rules {
        scanned |= read_generally(&mut rule.expression);
    }
    let mut programs = vec![("interpreted", Program::new(plan, start))];
    if scanned {
        programs.push(("read generally", Program::new(general, start)));
    }
    // Interpreted rules take more stack than generated ones.
    thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let depth = caiaphas::__private::DEPTH;
            for input in all {
                let expected = (generated.parse)(&input);
                let recovered =
                    recovers.then(|| (generated.recovering)(&input));
                for (way, program) in &programs {
                    assert_eq!(
                        program.parse(&input, depth),
                        expected,
                        "input {input:?}, {way}"
                    );
                    if let Some(recovered) = &recovered {
                        assert_eq!(
                            &program.parse_recovering(&input, depth),
                            recovered,
                            "input {input:?}, {way}, recovering"
                        );
                    }
                }
                // The first error a parse that recovers finds is the one a
                // parse that does not gives.
                if let Some((_, errors)) = &recovered {
                    assert_eq!(
                        errors.first(),
                        expected.as_ref().err(),
                        "input {input:?}"
                    );
                }
            }
        })
        .expect("a thread can start")
        .join()
        .expect("the interpreter matches as the generated parser does");
}

/// Makes each exception in `part` read as any other, and returns whether one
/// was read in one scan of a run.
fn read_generally(part: &mut Part) -> bool {
    let mut scanned = mem::take(&mut part.before_middle);
    match &mut part.kind {
        Kind::Sequence(parts) | Kind::Choice(parts) => {
            for part in parts {
                scanned |= read_generally(part);
            }
        }
        Kind::Optional(part) | Kind::Repeat(part) | Kind::Times(_, part) => {
            scanned |= read_generally(part);
        }
        Kind::Except { factor, exception } => {
            scanned |= read_generally(factor);
            scanned |= read_generally(exception);
        }
        _ => {}
    }
    scanned
}

/// A test for each grammar file and rule a parse starts from, comparing the
/// interpreter with the parser `grammar!` generates for it. The module of each
/// grammar takes the name of its test.
macro_rules! files {
    ($($test:ident = $path:literal, $start:literal => $ty:ident;)*) => {
        $(
            caiaphas::grammar!(mod $test = $path);

            #[test]
            fn $test() {
                let generated = Generated {
                    parse: $test::$ty::__tree,
                    recovering: $test::$ty::__tree_recovering,
                };
                compare(
                    include_str!(concat!("../", $path)),
                    $start,
                    generated,
                    0x9E37_79B9_7F4A_7C15,
                    &[],
                );
            }
        )*
    };
}

files! {
    arithmetic = "../tests/grammars/arithmetic.ebnf",
        "arithmetic" => Arithmetic;
    declarations = "../tests/grammars/declarations.ebnf",
        "program" => Program;
    exclamations = "../tests/grammars/exclamations.ebnf",
        "exclamation" => Exclamation;
    going_back = "../tests/grammars/going_back.ebnf",
        "maybe" => Maybe;
    greedy = "../tests/grammars/greedy.ebnf",
        "phrase" => Phrase;
    hidden = "../tests/grammars/hidden.ebnf",
        "pair" => Pair;
    iso14977 = "../tests/grammars/iso14977.ebnf",
        "syntax" => Syntax;
    items = "../tests/grammars/items.ebnf",
        "items" => Items;
    json = "../tests/grammars/json.ebnf",
        "json" => Json;
    json_value = "../tests/grammars/json.ebnf",
        "value" => Value;
    letter_x = "../tests/grammars/letter_x.ebnf",
        "letter x" => LetterX;
    list = "../tests/grammars/list.ebnf",
        "numbers" => Numbers;
    marked = "../tests/grammars/marked.ebnf",
        "marked" => Marked;
    phrase = "../tests/grammars/phrase.ebnf",
        "phrase" => Phrase;
    shared = "../tests/grammars/shared.ebnf",
        "shared" => Shared;
    split = "../tests/grammars/split.ebnf",
        "split" => Split;
    starts = "../tests/grammars/starts.ebnf",
        "starts" => Starts;
    toml = "../tests/grammars/toml.ebnf",
        "toml" => Toml;
    twice = "../tests/grammars/twice.ebnf",
        "twice" => Twice;
    words = "../tests/grammars/words.ebnf",
        "word" => Word;
    calculator = "../examples/grammars/calculator.ebnf",
        "calculation" => Calculation;
    ini = "../examples/grammars/ini.ebnf",
        "ini" => Ini;
    json_example = "../examples/grammars/json.ebnf",
        "json" => Json;
}

/// A test for each grammar written in the macro, comparing the interpreter
/// with the parser `grammar!` generates for it, also on the inputs written
/// after it, if any. The module of each grammar takes the name of its test.
macro_rules! written {
    ($(
        $test:ident { $grammar:tt } $([$($input:literal),* $(,)?])?,
        $start:literal => $ty:ident;
    )*) => {
        $(
            caiaphas::grammar!(mod $test { $grammar });

            #[test]
            fn $test() {
                let generated = Generated {
                    parse: $test::$ty::__tree,
                    recovering: $test::$ty::__tree_recovering,
                };
                let written = [$($($input),*)?];
                let seed = 0x2545_F491_4F6C_DD1D;
                compare($grammar, $start, generated, seed, &written);
            }
        )*
    };
}

written! {
    assignment {
        "assignment = name, '=', name; name = letter, {letter};
         (* hide *) letter = ? U+0061..U+007A ?;"
    }, "assignment" => Assignment;
    grouped {
        "sum = group, {'+', group};
         (* inline *) group = '(', number, ')' | number;
         number = digit, {digit};
         (* hide *) digit = ? U+0030..U+0039 ?;"
    }, "sum" => Sum;
    remembered {
        "pair = both, 'x' | both, 'y';
         (* inline *) both = a, b; a = 'a'; b = 'b';"
    }, "pair" => Pair;
    parenthesized {
        "sum = primary, {'+', primary};
         primary = number | group;
         (* inline *) group = '(', sum, ')';
         number = ? /[0-9]+/ ?;"
    }, "sum" => Sum;
    closed {
        "group = '(', 'x', close; (* hide *) close = ')', ';';"
    }, "group" => Group;
    settings {
        "settings = setting, {';', setting};
         setting = name, '=', value;
         (* expect tied-branches: a flag is never a word *)
         value = number | flag | word;
         (* as u8 *) number = digit, {digit};
         (* as bool *) flag = 'true' | 'false';
         (* as String *) word = letter, {letter};
         (* as String *) name = letter, {letter};
         (* hide *) letter = ? U+0061..U+007A ?;
         (* hide *) digit = ? U+0030..U+0039 ?;"
    }, "settings" => Settings;
    price {
        "price = amount, ' ', currency;
         (* as f64 *) amount = digit, {digit}, ['.', digit, {digit}];
         currency = 'EUR' | 'USD';
         (* hide *) digit = ? U+0030..U+0039 ?;"
    }, "price" => Price;
    bytes {
        "(* expect ambiguous-split: the first byte takes what it can *)
         pair = byte, byte; (* as u8 *) byte = digit, {digit};
         (* hide *) digit = ? U+0030..U+0039 ?;"
    }, "pair" => Pair;
    leftover {
        "(* greedy *) s = head, c;
         head = [d], 'a', {'x'}, 'x';
         d = c, 'y';
         c = ? U+0061..U+0062 ?;"
    }, "s" => S;
    leftover_variant {
        "(* greedy *) e = head | k;
         head = [d], 'q', {'x'}, 'x';
         d = c, 'y';
         c = 'q';
         k = 'w';"
    }, "e" => E;
    ends_apart {
        "(* expect tied-branches: the first wins *)
         s = a, 'ab' | a, 'b'; a = {'a'};"
    }, "s" => S;
    converted_character {
        "s = {c}, 'x';
         (* hide *) (* as u8 *) c = ? U+0030..U+0039 ? | 'a';"
    }, "s" => S;
    statement {
        "statement = name, ' = ', name;
         name = letters - keyword;
         (* hide *) letters = letter, {letter};
         (* hide *) keyword = 'let' | 'if';
         (* hide *) letter = ? U+0061..U+007A ?;"
    }, "statement" => Statement;
    hidden_ranges {
        "pair = 'x', (tail | single);
         (* hide *) tail = ? U+0061..U+007A ?, '!';
         (* hide *) single = ? U+0030..U+0039 ? | ',';"
    }, "pair" => Pair;
    comment {
        "comment = '/*', body, '*/';
         (* as String *) body = {character} - ({character}, '*/', {character});
         (* hide *) character = ? U+0020..U+007E ?;"
    }, "comment" => Comment;
    address {
        "address = byte, '.', byte, '.', byte, '.', byte;
         (* hide *) (* as u8 *) byte = ? /\\d+/ ?;"
    }, "address" => Address;
    either_bracket {
        "brackets = '(', [brackets], ')' | '(', [brackets], ')', ';';"
    }, "brackets" => Brackets;
    spaced_words {
        "(* expect ambiguous-split: the first word takes what it can *)
         words = word, {' ', word, ';'}, [' '];
         word = letter, {letter | ' '};
         (* hide *) letter = ? U+0061..U+007A ?;"
    }, "words" => Words;
    file {
        "file = name, '.', extension;
         name = character, {character};
         extension = letter, {letter};
         (* hide *) character = letter | '.';
         (* hide *) letter = ? U+0061..U+007A ?;"
    }, "file" => File;
    longest_first {
        "(* expect ambiguous-split: the first part takes what it can *)
         s = a, b; a = 'x' | 'x', 'y'; b = ['y'], 'z';"
    }, "s" => S;
    settings_recovering {
        r"settings = [line], {newline, [line]};
          (* recover *)
          line = name, ws, '=', ws, value;
          (* as String *) name = ? /[a-z_]+/ ?;
          (* as String *) value = ? /[^\n ][^\n]*/ ?;
          (* hide *) ws = {' '};
          (* hide *) newline = ? U+000A ?;"
    }, "settings" => Settings;
    files_recovering {
        "files = file, {',', file};
         (* recover *)
         file = name, '.', extension;
         name = character, {character};
         extension = letter, {letter};
         (* hide *) character = letter | '.';
         (* hide *) letter = ? U+0061..U+007A ?;"
    }, "files" => Files;
    json_recovering {
        r#"json = ws, value, ws;
         value = object | array | number | 'true';
         object = '{', ws, [member, ws, {',', ws, member, ws}], '}';
         (* recover *)
         member = string, ws, ':', ws, value;
         array = '[', ws, [value, ws, {',', ws, value, ws}], ']';
         string = '"', ? /[a-z]*/ ?, '"';
         number = ? /\d+/ ?;
         (* hide *) ws = {' '};"#
    }, "json" => Json;
    comments {
        "s = {'(*', text, '*)', ' '};
         (* hide *) text = {c} - ({c}, ('(*' | '*)'), {c});
         (* hide *) c = ? /[ab*()]/ ?;"
    } [
        "(**) ", "(*a*)b*) ", "(*(*a*) ", "(*a)*) (*b*) ", "(*ab", "(**)*) ",
        "(*a*b(a)*) ",
    ], "s" => S;
    quoted {
        "s = {(quoted | 'x'), ','};
         quoted = '<', body, '>';
         body = {l} - ({any}, close, {any});
         (* hide *) close = '>' | 'ab', ['c'] | 'cx>';
         (* hide *) l = ? /[a-c>U+00E9]/ ?;
         (* hide *) any = ? /[a-zU+00E9<>]/ ?;"
    } [
        "<cx>,", "<acx>,<b>,", "<cxa>,", "x,<c>,x,", "<ab>,", "<abc>,",
        "<a>b>,", "<\u{e9}c>,",
    ], "s" => S;
    strings {
        r#"s = {'"', body, '"', ','};
           body = {c} - ({c}, 'ab', {c});
           (* hide *) c = ? /[abc]/ ?;"#
    } [
        r#""cab","#, r#""ca","b","#, r#""","#, r#""aab""#, r#""cba","#,
    ], "s" => S;
    leaf_recovering {
        "list = item, {';', item};
         (* recover *) (* hide *) item = 'a', 'b';"
    }, "list" => List;
    empty_rounds {
        "s = {['x']}, ';', {'y' | }, '.';"
    } [";.", "x;.", "xx;y.", "xxx;yy.", ";", "x"], "s" => S;
    empty_round_nodes {
        "(* expect ambiguous-split: each round takes what it can *)
         s = {a}, ';'; a = ['x'], ['y'];"
    } [";", "x;", "xy;", "yx;", "xyxy;", "y"], "s" => S;
    spanned_values {
        "s = {number, ' '};
         (* as caiaphas::Spanned<u8> *) number = ? /[0-9]+/ ?;"
    } ["1 255 ", "256 ", "12 300 "], "s" => S;
    not_keywords {
        "s = {statement};
         statement = keyword, [' '], name, ';' | name, ';';
         keyword = ('if' | 'i'), ? not /[a-f]/ ?;
         name = ? /[a-f]+/ ?;"
    } ["ifa;", "if a;", "i f;", "if;", "if ;", "ia;i b;"], "s" => S;
    not_lines {
        r"log = {line, line end};
          line = ? /[ab]+/ ?;
          (* hide *) line end = newline | ? end ?;
          (* hide *) newline = ? U+000A ?;"
    } ["ab\nb", "ab\n", "a\n\nb", "", "\n"], "log" => Log;
    not_pattern {
        "s = {item, ';'};
         item = 'a', ? not /b{2}|c/ ?, {'b'} | 'abb', 'c';"
    } ["a;", "ab;", "abb;", "abbc;", "ac;", "abbb;"], "s" => S;
    not_plain {
        "s = {'a', ? not /bc/ ?, ['b', 'd']}, ';';"
    } ["abd;", "a;", "abc;", "aabd;", "ab;"], "s" => S;
    not_exception {
        "s = {word, ' '};
         word = name - (keyword | name, ? end ?, 'x');
         keyword = ('if' | 'do'), ? not /[a-z]/ ?;
         name = ? /[a-z]+/ ?;"
    } ["if ", "ifx ", "x ", "do do ", "dox if "], "s" => S;
    not_recovering {
        "settings = {line};
         (* recover *)
         line = name, '=', name, (';' | ? end ?);
         name = ? /[ab]+/ ?;"
    } ["a=b;b=a", "a=b;a;b=a", "a=b b=a", "a="], "settings" => Settings;
}
