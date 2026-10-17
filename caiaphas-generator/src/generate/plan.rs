//! Plans tell how a grammar's parser matches, as data. Each holds every choice
//! the code generation makes about each rule and expression, so a program can
//! run a grammar on an input the way its generated parser would, without
//! building anything.
//!
//! What a plan holds can change in any release, so only tools that come with
//! caiaphas read it.

use crate::{
    analysis::{CharacterSet, Facts, general_rules},
    compile::compile,
    diagnostic::{Diagnostic, Severity},
    generate::{
        clashes,
        exact::ExactWriter,
        matching::{Decisions, FunctionWriter},
        name,
        shape::{self, Shape},
    },
    ir::{Expression, ExpressionKind, Grammar, RuleId},
};

pub use crate::generate::matching::Expectation;

/// How a grammar's parser matches.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Plan {
    /// Every rule, by its number.
    pub rules: Vec<RulePlan>,
    /// The number of each start rule, the first written first.
    pub starts: Vec<usize>,
}

/// How a rule matches, and what its parser knows about it.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct RulePlan {
    /// The rule's name as errors give it.
    pub name: String,
    /// Whether the rule is left out of the types and mostly out of errors.
    pub hidden: bool,
    /// Whether the rule builds a node, since a type reads it.
    pub builds: bool,
    /// Whether what the rule could not read goes unsaid.
    pub quiet: bool,
    /// Whether the rule's node stands for its children.
    pub inline: bool,
    /// Whether no other rule uses the rule.
    pub start: bool,
    /// Whether a rule that takes each way as it comes remembers its match at
    /// each position.
    pub remembers: bool,
    /// Whether the rule builds no node and reads no rule that does.
    pub leaf: bool,
    /// Whether the rule can reach itself.
    pub recursive: bool,
    /// Whether the rule tries every way through it.
    pub general: bool,
    /// Whether the rule's choices take their first alternative that matches.
    pub first_match: bool,
    /// The Rust type the rule's text converts to, as the grammar writes it.
    pub converts_to: Option<String>,
    /// What the rule matches.
    pub expression: Part,
}

/// An expression of a rule, with what its parser decides about it.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct Part {
    /// What the part is.
    pub kind: Kind,
    /// The characters the part matches and what a parser notes where it fails,
    /// when it only ever matches one character and builds no node.
    pub set: Option<Set>,
    /// The characters the part can start with.
    pub first: Vec<(char, char)>,
    /// Whether the part can match nothing.
    pub nullable: bool,
    /// For a choice, whether at most one alternative can match at any place.
    pub start_apart: bool,
    /// Whether the part reads a rule that keeps the order a first match gives
    /// its ends.
    pub keeps_order: bool,
    /// For an exception, whether what follows the text it reads cannot change
    /// where it ends.
    pub settled: bool,
    /// Whether the part builds the node of any rule.
    pub builds_nodes: bool,
    /// Whether the part has at most one end at any position, in a rule that
    /// tries every way.
    pub has_one_end: bool,
    /// For an exception, the characters it matches when it only ever matches
    /// one.
    pub exception_characters: Option<Vec<(char, char)>>,
}

/// The characters a part matches, and what a parser notes where it fails.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Set {
    /// The ranges of characters, sorted and apart.
    pub ranges: Vec<(char, char)>,
    /// What a parser notes as expected where none of them comes.
    pub expected: Vec<Expectation>,
}

/// What a part is.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Kind {
    /// These parts, one after another.
    Sequence(Vec<Part>),
    /// One of these alternatives.
    Choice(Vec<Part>),
    /// The part, or nothing.
    Optional(Box<Part>),
    /// The part any number of times.
    Repeat(Box<Part>),
    /// The part this many times.
    Times(u32, Box<Part>),
    /// What `factor` matches and `exception` does not.
    Except {
        factor: Box<Part>,
        exception: Box<Part>,
    },
    /// The rule with this number.
    Rule(usize),
    /// This text.
    Text(String),
    /// Any one character from the first to the last.
    Range(char, char),
    /// Nothing.
    Empty,
}

/// Returns the plan of the grammar `source`, or every problem that stops it as
/// the `grammar!` macro would, rendered with `path` naming its file.
///
/// # Errors
///
/// Returns each problem as text that shows where in the grammar it is.
pub fn of(source: &str, path: &str) -> Result<Plan, Vec<String>> {
    let render = |diagnostics: &[Diagnostic]| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(path, source, true))
            .collect::<Vec<String>>()
    };
    let (grammar, diagnostics) = compile(source, false);
    let problems: Vec<Diagnostic> = diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity != Severity::Suggestion)
        .collect();
    let Some(grammar) = grammar.filter(|_| problems.is_empty()) else {
        return Err(render(&problems));
    };
    let shapes = shape::shapes(&grammar);
    let clashes = clashes(&grammar, &shapes);
    if !clashes.is_empty() {
        return Err(render(&clashes));
    }
    Ok(plan(&grammar, &shapes))
}

/// Returns the plan of `grammar`, whose types `shapes` tells.
fn plan(grammar: &Grammar, shapes: &[Option<Shape>]) -> Plan {
    let facts = Facts::new(grammar);
    let general = general_rules(&facts);
    let builds: Vec<bool> = grammar
        .syntax_rules
        .iter()
        .zip(shapes)
        .map(|(rule, shape)| !rule.hidden && (shape.is_some() || rule.inline))
        .collect();
    let decisions = Decisions::new(&facts, &general, &builds);
    let rules = grammar
        .syntax_rules
        .iter()
        .enumerate()
        .map(|(position, rule)| {
            let writer = ExactWriter {
                writer: FunctionWriter {
                    facts: &facts,
                    builds: &builds,
                    first_match: rule.first_match,
                },
                general: &general,
                ordered: &decisions.ordered,
                settled: &decisions.settled,
                recursive: &decisions.recursive,
            };
            let rule_id = RuleId(position);
            RulePlan {
                name: name::spaced(rule),
                hidden: rule.hidden,
                builds: builds[position],
                quiet: rule.hidden && facts.is_nullable(&rule.expression),
                inline: rule.inline,
                start: grammar.start_syntax_rules.contains(&rule_id),
                // A rule that tries every way remembers its ends.
                remembers: decisions.remembers[position] || general[position],
                leaf: decisions.leaves[position],
                recursive: decisions.recursive[position],
                general: general[position],
                first_match: rule.first_match,
                converts_to: rule.converts_to.clone(),
                expression: part(&writer, &rule.expression),
            }
        })
        .collect();
    let starts = grammar
        .start_syntax_rules
        .iter()
        .map(|&RuleId(position)| position)
        .collect();
    Plan { rules, starts }
}

/// Returns the ranges of `characters`.
fn ranges(characters: &CharacterSet) -> Vec<(char, char)> {
    characters
        .ranges()
        .iter()
        .map(|range| (*range.start(), *range.end()))
        .collect()
}

/// Returns the plan of `expression`, with the decisions `writer` makes.
fn part(writer: &ExactWriter<'_>, expression: &Expression) -> Part {
    let facts = writer.writer.facts;
    let boxed = |expression| Box::new(part(writer, expression));
    let kind = match &expression.kind {
        ExpressionKind::SingleDefinition(steps) => Kind::Sequence(
            steps.iter().map(|step| part(writer, step)).collect(),
        ),
        ExpressionKind::DefinitionsList(alternatives) => Kind::Choice(
            alternatives
                .iter()
                .map(|alternative| part(writer, alternative))
                .collect(),
        ),
        ExpressionKind::OptionalSequence(sequence) => {
            Kind::Optional(boxed(sequence))
        }
        ExpressionKind::RepeatedSequence(sequence) => {
            Kind::Repeat(boxed(sequence))
        }
        ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary,
        } => Kind::Times(*integer, boxed(syntactic_primary)),
        ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } => Kind::Except {
            factor: boxed(syntactic_factor),
            exception: boxed(syntactic_exception),
        },
        ExpressionKind::MetaIdentifier(RuleId(position)) => {
            Kind::Rule(*position)
        }
        ExpressionKind::TerminalString(text) => Kind::Text(text.clone()),
        ExpressionKind::SpecialSequence(range) => {
            Kind::Range(*range.start(), *range.end())
        }
        ExpressionKind::EmptySequence => Kind::Empty,
    };
    let (start_apart, settled, exception_characters) = match &expression.kind {
        ExpressionKind::DefinitionsList(alternatives) => {
            (facts.start_apart(alternatives), false, None)
        }
        ExpressionKind::SyntacticTerm {
            syntactic_exception,
            ..
        } => {
            let mut settled = true;
            syntactic_exception.visit_meta_identifiers(
                &mut |RuleId(read), _| settled &= writer.settled[read],
            );
            let characters = facts
                .single_characters(syntactic_exception)
                .map(|characters| ranges(&characters));
            (false, settled, characters)
        }
        _ => (false, false, None),
    };
    Part {
        kind,
        set: writer.writer.expectations(expression).map(
            |(characters, expected)| Set {
                ranges: ranges(&characters),
                expected,
            },
        ),
        first: ranges(&facts.first(expression)),
        nullable: facts.is_nullable(expression),
        start_apart,
        keeps_order: writer.keeps_order(expression),
        settled,
        builds_nodes: writer.builds_nodes(expression),
        has_one_end: writer.has_one_end(expression),
        exception_characters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_each_rule_with_what_its_parser_decides() {
        let plan = of(
            "file = name, '.', extension;\n\
             name = character, {character};\n\
             extension = letter, {letter};\n\
             (* hide *) character = letter | '.';\n\
             (* hide *) letter = ? U+0061..U+007A ?;",
            "file.ebnf",
        )
        .unwrap();
        let names: Vec<&str> =
            plan.rules.iter().map(|rule| rule.name.as_str()).collect();
        assert_eq!(
            names,
            ["file", "name", "extension", "character", "letter"]
        );
        assert_eq!(plan.starts, [0]);
        assert!(plan.rules[1].general, "a name can hold dots");
        assert!(!plan.rules[2].general);
        let character = &plan.rules[3];
        assert!(character.hidden && !character.builds);
        let set = character.expression.set.as_ref().unwrap();
        assert_eq!(set.ranges, [('.', '.'), ('a', 'z')]);
        assert_eq!(
            set.expected,
            [
                Expectation::Rule("letter".to_owned()),
                Expectation::Text(".".to_owned())
            ]
        );
    }

    #[test]
    fn gives_the_problems_that_stop_a_grammar() {
        let problems = of("s = a;", "s.ebnf").unwrap_err();
        assert!(problems[0].starts_with("error: `a` is not defined"));
    }
}
