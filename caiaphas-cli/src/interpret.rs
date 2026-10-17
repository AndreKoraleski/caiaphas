//! Interpreting runs a grammar on an input without building anything. It
//! follows the grammar's plan, holding every choice the code generation makes,
//! and calls the parser of the runtime as the generated code would, so it
//! matches, builds its tree and tells its errors as the generated parser does.
//!
//! A conversion to a type of the program's own cannot be checked here, since
//! the interpreter does not have the type, so such a rule takes any text its
//! grammar matches.

mod exact;
mod plain;

use std::cell::Cell;

use caiaphas::{
    __private::{
        self as runtime, Expected, Match, Parser, Rule, Start, Width,
    },
    Error,
};
use caiaphas_generator::plan::{Expectation, Kind, Part, Plan, RulePlan, Set};

/// A grammar's plan ready to run.
pub(crate) struct Program {
    plan: &'static Plan,
    /// What the parser knows about each rule, by its number.
    rules: Vec<Rule>,
    /// The number of the rule a parse starts from.
    start: usize,
}

thread_local! {
    /// The program a parse on this thread runs, since the runtime names the
    /// start of a parse by a type.
    static RUNNING: Cell<Option<&'static Program>> = const { Cell::new(None) };
}

impl Program {
    /// Returns the program of `plan`, starting from the rule numbered `start`.
    /// The plan and the program live as long as the process does, since the
    /// parser of the runtime keeps the names it is given.
    pub(crate) fn new(plan: Plan, start: usize) -> &'static Self {
        let plan: &'static Plan = Box::leak(Box::new(plan));
        let rules = plan
            .rules
            .iter()
            .enumerate()
            .map(|(number, rule)| Rule {
                number,
                name: rule.name.as_str(),
                hidden: rule.hidden,
                builds: rule.builds,
                quiet: rule.quiet,
                inline: rule.inline,
                remembers: rule.remembers,
                start: rule.start,
            })
            .collect();
        Box::leak(Box::new(Self { plan, rules, start }))
    }

    /// Returns the tree of `input` as text, as a node on each line, or why it
    /// does not match. Matches nest at most `depth` deep.
    pub(crate) fn parse(
        &'static self,
        input: &str,
        depth: usize,
    ) -> Result<String, Error> {
        RUNNING.with(|running| running.set(Some(self)));
        let names: Vec<&str> = self
            .plan
            .rules
            .iter()
            .map(|rule| rule.name.as_str())
            .collect();
        let parsed = runtime::parse::<Running>(input, depth);
        RUNNING.with(|running| running.set(None));
        parsed.map(|tree| tree.dump(&names))
    }

    /// Returns the type of each conversion the interpreter cannot check.
    pub(crate) fn unchecked(&self) -> Vec<&str> {
        let mut unchecked: Vec<&str> = self
            .plan
            .rules
            .iter()
            .filter_map(|rule| rule.converts_to.as_deref())
            .filter(|ty| !checks(ty))
            .collect();
        unchecked.sort_unstable();
        unchecked.dedup();
        unchecked
    }

    fn plan_of(&self, rule: usize) -> &'static RulePlan {
        &self.plan.rules[rule]
    }

    /// Matches the rule numbered `id` at `position`, as its function does: a
    /// rule that tries every way takes the end it prefers.
    fn rule<const R: bool, W: Width>(
        &self,
        parser: &mut Parser<'_, R, W>,
        id: usize,
        position: usize,
    ) -> Match {
        let plan = self.plan_of(id);
        if plan.general {
            let end = self.ends_rule(parser, id, position).first()?;
            let node = if plan.hidden {
                None
            } else {
                self.build_rule(parser, id, position, end)
            };
            return Some((end, node));
        }
        if plan.recursive {
            if !parser.enter(position) {
                return None;
            }
            let found = self.plain_rule(parser, id, position);
            parser.leave();
            return found;
        }
        self.plain_rule(parser, id, position)
    }
}

/// The start of a parse of the program running on this thread.
struct Running;

impl Start for Running {
    fn root<W: Width>(parser: &mut Parser<'_, false, W>) -> Option<usize> {
        let program = running();
        let start = program.start;
        let length = parser.length();
        if program.plan_of(start).general {
            if program.ends_rule(parser, start, 0).contains(length) {
                return program.build_rule(parser, start, 0, length);
            }
            return None;
        }
        let found = program.rule(parser, start, 0);
        runtime::whole(found, length)
    }

    fn explain(parser: &mut Parser<'_, true, u64>) -> Vec<usize> {
        let program = running();
        let start = program.start;
        if program.plan_of(start).general {
            return program.ends_rule(parser, start, 0).iter().collect();
        }
        program
            .rule(parser, start, 0)
            .map(|(end, _)| end)
            .into_iter()
            .collect()
    }
}

/// Returns the program running on this thread.
fn running() -> &'static Program {
    RUNNING.with(Cell::get).expect("a parse runs a program")
}

/// A branch of a choice: an alternative, or a run of alternatives that each
/// only match one character, read as one set.
enum Branch<'p> {
    Part(&'p Part),
    Sets(Vec<&'p Set>),
}

/// Returns the branches of a choice among `alternatives`.
fn branches(alternatives: &'static [Part]) -> Vec<Branch<'static>> {
    let mut branches = Vec::new();
    let mut run: Vec<&'static Part> = Vec::new();
    for alternative in alternatives {
        if set_parts(alternative).is_some() {
            run.push(alternative);
            continue;
        }
        flush(&mut run, &mut branches);
        branches.push(Branch::Part(alternative));
    }
    flush(&mut run, &mut branches);
    branches
}

/// Adds the alternatives of `run` to `branches`, as one set when there are
/// several.
fn flush<'p>(run: &mut Vec<&'p Part>, branches: &mut Vec<Branch<'p>>) {
    match run.as_slice() {
        [] => {}
        [alternative] => branches.push(Branch::Part(alternative)),
        _ => branches.push(Branch::Sets(
            run.iter()
                .map(|alternative| {
                    alternative.set.as_ref().expect("a run is of sets")
                })
                .collect(),
        )),
    }
    run.clear();
}

/// Returns the set of `part` when it only matches one character of a set that
/// is not empty, as a repetition or a run reads it.
fn set_parts(part: &'static Part) -> Option<&'static Set> {
    part.set.as_ref().filter(|set| !set.ranges.is_empty())
}

/// Returns whether a parse that only matches skips the branch `part` at
/// `position`, since the next character is none it can start with. A part that
/// `checks_itself` on its first character is never skipped.
fn skips<const R: bool, W: Width>(
    parser: &Parser<'_, R, W>,
    part: &'static Part,
    checks_itself: bool,
    position: usize,
) -> bool {
    if R || checks_itself
        || part.nullable
        || matches!(part.kind, Kind::Text(_) | Kind::Range(..))
    {
        return false;
    }
    !parser
        .character(position)
        .is_some_and(|c| within(&part.first, c))
}

/// Matches `part` as one check, when it only matches one character of `set`.
fn single<const R: bool, W: Width>(
    parser: &mut Parser<'_, R, W>,
    part: &'static Part,
    set: &'static Set,
    position: usize,
) -> Option<usize> {
    match &part.kind {
        Kind::Text(text) => parser.text(position, text),
        Kind::Range(first, last) => parser.range(position, *first, *last),
        _ => parser.set(
            position,
            contains(&set.ranges),
            &expected(&set.expected),
        ),
    }
}

/// Returns the set of `factor` and the characters its exception `part` leaves
/// out, when both only match one character, so one check matches them.
fn sets_apart(
    part: &'static Part,
    factor: &'static Part,
) -> Option<(&'static Set, &'static [(char, char)])> {
    Some((factor.set.as_ref()?, part.exception_characters.as_ref()?))
}

/// Matches one character of `set` that is none of `excludes`.
fn set_except<const R: bool, W: Width>(
    parser: &mut Parser<'_, R, W>,
    set: &'static Set,
    excludes: &'static [(char, char)],
    position: usize,
) -> Option<usize> {
    parser.set_except(
        position,
        contains(&set.ranges),
        contains(excludes),
        &expected(&set.expected),
    )
}

/// Returns whether `c` is in one of `ranges`.
fn within(ranges: &[(char, char)], c: char) -> bool {
    ranges.iter().any(|&(first, last)| first <= c && c <= last)
}

/// Returns the check of whether a character is in one of `ranges`.
fn contains(ranges: &[(char, char)]) -> impl Fn(char) -> bool + '_ {
    move |c| within(ranges, c)
}

/// Returns what a parser notes, from what a plan says it notes.
fn expected(expectations: &'static [Expectation]) -> Vec<Expected> {
    expectations
        .iter()
        .map(|expectation| match expectation {
            Expectation::Text(text) => Expected::Text(text),
            Expectation::Range(first, last) => Expected::Range(*first, *last),
            Expectation::Rule(name) => Expected::Rule(name),
        })
        .collect()
}

/// The paths of the standard library a conversion can name a type by.
const PATHS: &[&str] = &[
    "std::string::",
    "alloc::string::",
    "std::primitive::",
    "core::primitive::",
    "std::net::",
    "core::net::",
    "std::path::",
];

/// Returns `ty` without the path to it in the standard library.
fn last_part(ty: &str) -> &str {
    let ty = ty.strip_prefix("::").unwrap_or(ty);
    PATHS
        .iter()
        .find_map(|path| ty.strip_prefix(path))
        .unwrap_or(ty)
}

/// Runs `$check` with each type the interpreter can check a conversion to.
macro_rules! checked_types {
    ($check:ident) => {
        $check!(
            "i8" => i8, "i16" => i16, "i32" => i32, "i64" => i64,
            "i128" => i128, "isize" => isize, "u8" => u8, "u16" => u16,
            "u32" => u32, "u64" => u64, "u128" => u128, "usize" => usize,
            "f32" => f32, "f64" => f64, "bool" => bool, "char" => char,
            "String" => String, "IpAddr" => std::net::IpAddr,
            "Ipv4Addr" => std::net::Ipv4Addr,
            "Ipv6Addr" => std::net::Ipv6Addr,
            "SocketAddr" => std::net::SocketAddr,
            "PathBuf" => std::path::PathBuf
        )
    };
}

/// Returns whether the interpreter can check a conversion to `ty`.
fn checks(ty: &str) -> bool {
    macro_rules! names {
        ($($name:literal => $ty:ty),*) => {
            matches!(last_part(ty), $($name)|*)
        };
    }
    checked_types!(names)
}

/// Returns whether the type `ty` reads the text from `position` to `end`, as
/// the parser checks a conversion. A type the interpreter does not have takes
/// any text.
fn accepts<const R: bool, W: Width>(
    parser: &mut Parser<'_, R, W>,
    rule: &Rule,
    ty: &str,
    position: usize,
    end: usize,
) -> bool {
    macro_rules! accept {
        ($($name:literal => $ty:ty),*) => {
            match last_part(ty) {
                $($name => parser.accepts::<$ty>(rule, position, end),)*
                _ => true,
            }
        };
    }
    checked_types!(accept)
}
