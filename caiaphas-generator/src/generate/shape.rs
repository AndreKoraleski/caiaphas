//! Shapes tell which Rust type each rule of a grammar becomes.
//!
//! A rule whose alternatives are each another rule or a word becomes an enum
//! with a variant for each. Any other rule becomes a struct with a field for
//! each rule it holds, typed by how many times it can hold it, and a rule it
//! always holds the same number of times becomes an array. Hidden rules get no
//! type and no field, and neither do rules that no start rule holds. A type
//! that could contain itself holds itself in a box.

use std::collections::HashSet;

use crate::{
    generate::name,
    ir::{Expression, ExpressionKind, Grammar, RuleId, reachable},
};

/// The type a rule becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Shape {
    Struct(Vec<Field>),
    Enum(Vec<Variant>),
}

/// A field of a struct, holding the type of another rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) rule: RuleId,
    pub(crate) count: Count,
    /// Whether the field holds its type in a box, since that type could
    /// contain this one.
    pub(crate) boxed: bool,
}

/// How many times a rule can hold another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Count {
    One,
    Optional,
    /// The same number of times, more than once, on every way through.
    Fixed(u8),
    Many,
}

/// The fewest times a rule holds another that count as many, so fixed counts
/// stay small enough for an array.
const MANY: u8 = 16;

/// A variant of an enum, for one alternative of its rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Variant {
    /// Holds the type of another rule, in a box when that type could contain
    /// this one.
    Rule { rule: RuleId, boxed: bool },
    /// Stands for a terminal string that is a word.
    Word(String),
}

/// Returns the shape of each rule's type, or `None` for a rule without one.
pub(crate) fn shapes(grammar: &Grammar) -> Vec<Option<Shape>> {
    let mut shapes: Vec<Shape> = grammar
        .syntax_rules
        .iter()
        .map(|rule| shape(grammar, &rule.expression))
        .collect();
    box_cycles(&mut shapes);
    let reached = reached(grammar, &shapes);
    shapes
        .into_iter()
        .zip(reached)
        .map(|(shape, reached)| reached.then_some(shape))
        .collect()
}

fn shape(grammar: &Grammar, expression: &Expression) -> Shape {
    variants(grammar, expression).map_or_else(
        || Shape::Struct(fields(grammar, expression)),
        Shape::Enum,
    )
}

/// Returns a variant for each alternative of `expression`, when each is a rule
/// with a type or a word, and no two variants would share a name.
fn variants(
    grammar: &Grammar,
    expression: &Expression,
) -> Option<Vec<Variant>> {
    let ExpressionKind::DefinitionsList(alternatives) = &expression.kind
    else {
        return None;
    };
    let mut names = HashSet::new();
    alternatives
        .iter()
        .map(|alternative| {
            let (variant, name) = match &alternative.kind {
                ExpressionKind::MetaIdentifier(rule_id) => {
                    let RuleId(position) = *rule_id;
                    let rule = &grammar.syntax_rules[position];
                    if rule.hidden {
                        return None;
                    }
                    let name = name::type_name(rule.meta_identifier.written());
                    (
                        Variant::Rule {
                            rule: *rule_id,
                            boxed: false,
                        },
                        name,
                    )
                }
                ExpressionKind::TerminalString(text) => {
                    (Variant::Word(text.clone()), name::word_variant(text)?)
                }
                _ => return None,
            };
            names.insert(name.to_string()).then_some(variant)
        })
        .collect()
}

/// Returns a field for each rule `expression` holds, in the order they first
/// appear.
fn fields(grammar: &Grammar, expression: &Expression) -> Vec<Field> {
    occurrences(grammar, expression)
        .into_iter()
        .map(|(rule, Occurrences { least, most })| Field {
            rule,
            count: if least == most && (2..MANY).contains(&most) {
                Count::Fixed(most)
            } else if most > 1 {
                Count::Many
            } else if least == 1 {
                Count::One
            } else {
                Count::Optional
            },
            boxed: false,
        })
        .collect()
}

/// How many times an expression holds a rule, at least and at most, where
/// [`MANY`] stands for that many or more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Occurrences {
    least: u8,
    most: u8,
}

/// Returns how many times `expression` holds each rule with a type, in the
/// order the rules first appear.
fn occurrences(
    grammar: &Grammar,
    expression: &Expression,
) -> Vec<(RuleId, Occurrences)> {
    let of = |expression| occurrences(grammar, expression);
    match &expression.kind {
        ExpressionKind::MetaIdentifier(rule_id) => {
            let RuleId(position) = *rule_id;
            if grammar.syntax_rules[position].hidden {
                return Vec::new();
            }
            vec![(*rule_id, Occurrences { least: 1, most: 1 })]
        }
        ExpressionKind::SingleDefinition(expressions) => {
            expressions.iter().fold(Vec::new(), |held, expression| {
                combine(held, of(expression), |first, second| Occurrences {
                    least: first.least.saturating_add(second.least).min(MANY),
                    most: first.most.saturating_add(second.most).min(MANY),
                })
            })
        }
        ExpressionKind::DefinitionsList(expressions) => expressions
            .iter()
            .map(of)
            .reduce(|held, other| {
                combine(held, other, |first, second| Occurrences {
                    least: first.least.min(second.least),
                    most: first.most.max(second.most),
                })
            })
            .unwrap_or_default(),
        ExpressionKind::OptionalSequence(expression) => {
            scaled(of(expression), |count| Occurrences { least: 0, ..count })
        }
        ExpressionKind::RepeatedSequence(expression) => {
            scaled(of(expression), |_| Occurrences {
                least: 0,
                most: MANY,
            })
        }
        ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary,
        } => {
            let times = u8::try_from(*integer).unwrap_or(MANY);
            scaled(of(syntactic_primary), |count| Occurrences {
                least: count.least.saturating_mul(times).min(MANY),
                most: count.most.saturating_mul(times).min(MANY),
            })
        }
        ExpressionKind::SyntacticTerm {
            syntactic_factor, ..
        } => of(syntactic_factor),
        ExpressionKind::TerminalString(_)
        | ExpressionKind::SpecialSequence(_)
        | ExpressionKind::EmptySequence => Vec::new(),
    }
}

/// Returns the rules of `first` and `second`, in the order they first appear,
/// with `merge` giving the count of a rule from both counts, where a rule that
/// one does not hold counts as held no times.
fn combine(
    first: Vec<(RuleId, Occurrences)>,
    second: Vec<(RuleId, Occurrences)>,
    merge: impl Fn(Occurrences, Occurrences) -> Occurrences,
) -> Vec<(RuleId, Occurrences)> {
    let none = Occurrences { least: 0, most: 0 };
    let count_in = |held: &[(RuleId, Occurrences)], rule: RuleId| {
        held.iter()
            .find(|(other, _)| *other == rule)
            .map_or(none, |&(_, count)| count)
    };
    let mut all: Vec<(RuleId, Occurrences)> = first
        .iter()
        .map(|&(rule, count)| (rule, merge(count, count_in(&second, rule))))
        .collect();
    for &(rule, count) in &second {
        if !first.iter().any(|&(other, _)| other == rule) {
            all.push((rule, merge(none, count)));
        }
    }
    all
}

fn scaled(
    held: Vec<(RuleId, Occurrences)>,
    scale: impl Fn(Occurrences) -> Occurrences,
) -> Vec<(RuleId, Occurrences)> {
    held.into_iter()
        .map(|(rule, count)| (rule, scale(count)))
        .collect()
}

/// Returns the rules whose types `shape` holds.
fn held(shape: &Shape) -> Vec<RuleId> {
    match shape {
        Shape::Struct(fields) => {
            fields.iter().map(|field| field.rule).collect()
        }
        Shape::Enum(variants) => variants
            .iter()
            .filter_map(|variant| match variant {
                Variant::Rule { rule, .. } => Some(*rule),
                Variant::Word(_) => None,
            })
            .collect(),
    }
}

/// Returns the rules whose types `shape` holds without a list in between, so
/// that a type holding itself that way would never end.
fn contained(shape: &Shape) -> Vec<RuleId> {
    match shape {
        Shape::Struct(fields) => fields
            .iter()
            .filter(|field| field.count != Count::Many)
            .map(|field| field.rule)
            .collect(),
        Shape::Enum(_) => held(shape),
    }
}

/// Boxes every field and variant whose type could contain the type it is in.
fn box_cycles(shapes: &mut [Shape]) {
    let contained: Vec<Vec<RuleId>> = shapes.iter().map(contained).collect();
    for (position, shape) in shapes.iter_mut().enumerate() {
        let contains_this =
            |rule: RuleId| reaches(&contained, rule, RuleId(position));
        match shape {
            Shape::Struct(fields) => {
                for field in fields {
                    field.boxed = field.count != Count::Many
                        && contains_this(field.rule);
                }
            }
            Shape::Enum(variants) => {
                for variant in variants {
                    if let Variant::Rule { rule, boxed } = variant {
                        *boxed = contains_this(*rule);
                    }
                }
            }
        }
    }
}

/// Returns whether the type of `from` contains the type of `to`, by itself or
/// through other types, without a list in between.
fn reaches(contained: &[Vec<RuleId>], from: RuleId, to: RuleId) -> bool {
    let RuleId(position) = to;
    reachable(contained.len(), [from], |RuleId(rule), pending| {
        pending.extend(&contained[rule]);
    })[position]
}

/// Returns which rules get a type, which are the start rules and the rules
/// their types hold.
fn reached(grammar: &Grammar, shapes: &[Shape]) -> Vec<bool> {
    let starts = grammar.start_syntax_rules.iter().copied();
    reachable(shapes.len(), starts, |RuleId(position), pending| {
        pending.extend(held(&shapes[position]));
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::tests::build_valid;

    /// Returns the shape of each rule of `source`, by name, with the rules it
    /// holds written by name too.
    fn shapes_of(source: &str) -> Vec<(String, Option<String>)> {
        let grammar = build_valid(source);
        let name = |RuleId(position): RuleId| {
            grammar.syntax_rules[position]
                .meta_identifier
                .as_str()
                .to_owned()
        };
        shapes(&grammar)
            .into_iter()
            .enumerate()
            .map(|(position, shape)| {
                let shape = shape.map(|shape| match shape {
                    Shape::Struct(fields) => {
                        let fields: Vec<String> = fields
                            .into_iter()
                            .map(|field| {
                                let boxed =
                                    if field.boxed { " boxed" } else { "" };
                                format!(
                                    "{} {:?}{boxed}",
                                    name(field.rule),
                                    field.count
                                )
                            })
                            .collect();
                        format!("struct {{{}}}", fields.join(", "))
                    }
                    Shape::Enum(variants) => {
                        let variants: Vec<String> = variants
                            .into_iter()
                            .map(|variant| match variant {
                                Variant::Rule { rule, boxed } => {
                                    let boxed =
                                        if boxed { " boxed" } else { "" };
                                    format!("{}{boxed}", name(rule))
                                }
                                Variant::Word(word) => format!("{word:?}"),
                            })
                            .collect();
                        format!("enum {{{}}}", variants.join(", "))
                    }
                });
                (name(RuleId(position)), shape)
            })
            .collect()
    }

    fn shape(name: &str, shape: &str) -> (String, Option<String>) {
        (name.to_owned(), Some(shape.to_owned()))
    }

    #[test]
    fn makes_structs_and_enums() {
        assert_eq!(
            shapes_of(
                "json = value; \
                 value = object | number | 'true' | 'null'; \
                 object = '{', [member, {',', member}], '}'; \
                 member = number, ':', value; \
                 number = ? U+0030..U+0039 ?, {? U+0030..U+0039 ?};"
            ),
            [
                shape("json", "struct {value One}"),
                shape("value", "enum {object, number, \"true\", \"null\"}"),
                shape("object", "struct {member Many}"),
                shape("member", "struct {number One, value One}"),
                shape("number", "struct {}"),
            ]
        );
    }

    #[test]
    fn counts_how_many_times_a_rule_is_held() {
        assert_eq!(
            shapes_of(
                "s = a, [a] | b, 2 * c, d - e; \
                 a = 'x'; b = 'y'; c = 'z'; d = 'w'; e = 'v';"
            ),
            [
                shape("s", "struct {a Many, b Optional, c Many, d Optional}"),
                shape("a", "struct {}"),
                shape("b", "struct {}"),
                shape("c", "struct {}"),
                shape("d", "struct {}"),
                ("e".to_owned(), None),
            ]
        );
    }

    #[test]
    fn holds_a_rule_held_the_same_number_of_times_in_an_array() {
        assert_eq!(
            shapes_of(
                "s = a, '=', a, 3 * b, 20 * c; a = 'x'; b = 'y'; c = 'z';"
            )[0],
            shape("s", "struct {a Fixed(2), b Fixed(3), c Many}")
        );
    }

    #[test]
    fn makes_a_rule_held_in_an_optional_sequence_optional() {
        assert_eq!(
            shapes_of("s = [a]; a = 'x';")[0],
            shape("s", "struct {a Optional}")
        );
    }

    #[test]
    fn boxes_a_type_that_could_contain_itself() {
        assert_eq!(
            shapes_of(
                "s = expression; expression = atom | group; \
                 group = '(', expression, ')'; atom = 'x';"
            ),
            [
                shape("s", "struct {expression One}"),
                shape("expression", "enum {atom, group boxed}"),
                shape("group", "struct {expression One boxed}"),
                shape("atom", "struct {}"),
            ]
        );
        assert_eq!(
            shapes_of("e = 'n' | '(', e, ')';"),
            [shape("e", "struct {e Optional boxed}")]
        );
    }

    #[test]
    fn needs_no_box_for_a_list() {
        assert_eq!(
            shapes_of("list = '(', {list}, ')';"),
            [shape("list", "struct {list Many}")]
        );
    }

    #[test]
    fn gives_no_type_or_field_to_hidden_rules_or_what_only_they_hold() {
        assert_eq!(
            shapes_of(
                "s = ws, 'x', ws; (* hide *) ws = {space}; space = ' ';"
            ),
            [
                shape("s", "struct {}"),
                ("ws".to_owned(), None),
                ("space".to_owned(), None),
            ]
        );
    }

    #[test]
    fn makes_a_struct_when_variants_would_share_a_name_or_have_none() {
        assert_eq!(
            shapes_of("s = object | 'object'; object = 'x';")[0],
            shape("s", "struct {object Optional}")
        );
        assert_eq!(
            shapes_of("s = a | '+'; a = 'x';")[0],
            shape("s", "struct {a Optional}")
        );
        assert_eq!(
            shapes_of("s = ws | a; (* hide *) ws = ' '; a = 'x';")[0],
            shape("s", "struct {a Optional}")
        );
    }
}
