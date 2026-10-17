//! Shapes tell which Rust type each rule of a grammar becomes.
//!
//! A rule whose alternatives are each another rule or a word becomes an enum
//! with a variant for each, and so does a group of rules among parts that hold
//! nothing, as in `(a | b), ws` with `ws` hidden. Any other rule becomes a
//! struct with a field for each rule it holds, typed by how many times it can
//! hold it, and a rule it always holds the same number of times becomes an
//! array. A rule that holds an inline rule holds what the inline rule holds
//! instead, as if its definition were written there, and when that is one
//! rule, the inline rule labels it: the field takes the inline rule's name, so
//! two places that hold the same rule can have fields of their own. Hidden and
//! inline rules get no type, and neither do rules that no start rule holds. A
//! type that could contain itself holds itself in a box.

mod counts;

use std::collections::HashSet;

use crate::{
    generate::name,
    ir::{Expression, ExpressionKind, Grammar, RuleId, reachable},
    span::Span,
};
use counts::{ONCE, Occurrences, occurrences};
pub(crate) use counts::{labels, spliced};

/// The type a rule becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Shape {
    Struct(Vec<Field>),
    Enum(Vec<Variant>),
    /// The Rust type a conversion names, read from the rule's text.
    Converted(String),
}

/// A field of a struct, holding the type of another rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) rule: RuleId,
    /// The inline rule that holds the rule here and names the field, if any.
    pub(crate) label: Option<RuleId>,
    pub(crate) count: Count,
    /// Whether the field holds its type in a box.
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
    /// Holds the type of another rule, in a box when `boxed` holds. It is
    /// named after `named`, the rule itself or an inline rule that holds it.
    Rule {
        rule: RuleId,
        named: RuleId,
        boxed: bool,
    },
    /// Stands for a terminal string that is a word.
    Word(String),
}

/// Returns the shape of each rule's type, or `None` for a rule without one.
pub(crate) fn shapes(grammar: &Grammar) -> Vec<Option<Shape>> {
    let labels = labels(grammar);
    let mut shapes: Vec<Shape> = grammar
        .syntax_rules
        .iter()
        .map(|rule| match &rule.converts_to {
            Some(ty) => Shape::Converted(ty.clone()),
            None => shape(grammar, &labels, &rule.expression),
        })
        .collect();
    box_cycles(&mut shapes);
    let reached = reached(grammar, &shapes);
    shapes
        .into_iter()
        .zip(reached)
        .zip(&grammar.syntax_rules)
        .map(|((shape, reached), rule)| {
            (reached && !rule.inline).then_some(shape)
        })
        .collect()
}

fn shape(
    grammar: &Grammar,
    labels: &[Option<RuleId>],
    expression: &Expression,
) -> Shape {
    variants(grammar, labels, expression)
        .or_else(|| grouped_variants(grammar, labels, expression))
        .map_or_else(
            || Shape::Struct(fields(grammar, labels, expression)),
            Shape::Enum,
        )
}

/// Returns the variants of a sequence whose one part that holds a rule is a
/// group of alternatives, as in `(a | b), ws` with `ws` hidden, when each of
/// them holds a rule. A word variant is told by the text of the whole match,
/// and the other parts add to that text, so a word leaves the rule a struct.
fn grouped_variants(
    grammar: &Grammar,
    labels: &[Option<RuleId>],
    expression: &Expression,
) -> Option<Vec<Variant>> {
    let ExpressionKind::SingleDefinition(parts) = &expression.kind else {
        return None;
    };
    let mut holding = parts
        .iter()
        .filter(|part| !occurrences(grammar, labels, part).is_empty());
    let group = holding.next()?;
    if holding.next().is_some()
        || !matches!(group.kind, ExpressionKind::DefinitionsList(_))
    {
        return None;
    }
    let variants = variants(grammar, labels, group)?;
    variants
        .iter()
        .all(|variant| matches!(variant, Variant::Rule { .. }))
        .then_some(variants)
}

/// Returns a variant for each alternative of `expression`, when each is a rule
/// with a type, an inline rule that labels one such rule held once, or a word,
/// and no two variants would share a name or the rule they hold.
fn variants(
    grammar: &Grammar,
    labels: &[Option<RuleId>],
    expression: &Expression,
) -> Option<Vec<Variant>> {
    let ExpressionKind::DefinitionsList(alternatives) = &expression.kind
    else {
        return None;
    };
    let mut names = HashSet::new();
    let mut held = HashSet::new();
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
                    let holds = if rule.inline {
                        // A variant takes the one rule a label holds.
                        let held =
                            occurrences(grammar, labels, &rule.expression);
                        match held[..] {
                            [(inner, ONCE)]
                                if labels[position] == Some(inner) =>
                            {
                                inner
                            }
                            _ => return None,
                        }
                    } else {
                        *rule_id
                    };
                    if !held.insert(holds.0) {
                        return None;
                    }
                    let name = name::type_name(rule.meta_identifier.written());
                    (
                        Variant::Rule {
                            rule: holds,
                            named: *rule_id,
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
/// appear. A label holds its rule as many times as it is held, times as many
/// times as it holds the rule.
fn fields(
    grammar: &Grammar,
    labels: &[Option<RuleId>],
    expression: &Expression,
) -> Vec<Field> {
    occurrences(grammar, labels, expression)
        .into_iter()
        .map(|(held, times)| {
            let RuleId(position) = held;
            let (rule, label, Occurrences { least, most }) = match labels
                [position]
            {
                Some(rule) => {
                    let inside = &grammar.syntax_rules[position].expression;
                    let each = occurrences(grammar, labels, inside)
                        .first()
                        .map_or(ONCE, |&(_, each)| each);
                    let product = |first: u8, second: u8| {
                        first.saturating_mul(second).min(MANY)
                    };
                    let times = Occurrences {
                        least: product(times.least, each.least),
                        most: product(times.most, each.most),
                    };
                    (rule, Some(held), times)
                }
                None => (held, None, times),
            };
            Field {
                rule,
                label,
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
            }
        })
        .collect()
}

/// A rule that places of one sequence hold a different number of times each,
/// in a struct whose field for it gathers every match.
pub(crate) struct Gathering {
    /// The rule of the struct.
    pub(crate) rule: RuleId,
    /// The rule the field holds, or the label that names it.
    pub(crate) held: RuleId,
    /// The sequence.
    pub(crate) sequence: Span,
    /// The parts of the sequence that hold the rule.
    pub(crate) places: Vec<Span>,
}

/// Returns each rule that places of a sequence hold a different number of
/// times each, in each struct of `grammar`. A list such as `item, {',', item}`
/// is one place, since the repetitions after the first place repeat it.
pub(crate) fn gatherings(grammar: &Grammar) -> Vec<Gathering> {
    let labels = labels(grammar);
    let mut gatherings = Vec::new();
    for (position, shape) in shapes(grammar).iter().enumerate() {
        if !matches!(shape, Some(Shape::Struct(_))) {
            continue;
        }
        let rule = &grammar.syntax_rules[position];
        rule.expression.visit(&mut |expression| {
            let ExpressionKind::SingleDefinition(steps) = &expression.kind
            else {
                return;
            };
            let held: Vec<_> = steps
                .iter()
                .map(|step| occurrences(grammar, &labels, step))
                .collect();
            let mut seen = Vec::new();
            for &(rule_id, _) in held.iter().flatten() {
                if seen.contains(&rule_id) {
                    continue;
                }
                seen.push(rule_id);
                let places: Vec<(&Expression, Occurrences)> = steps
                    .iter()
                    .zip(&held)
                    .filter_map(|(step, held)| {
                        let &(_, count) = held
                            .iter()
                            .find(|&&(other, _)| other == rule_id)?;
                        Some((step, count))
                    })
                    .collect();
                let Some(((_, first), later)) = places.split_first() else {
                    continue;
                };
                let list = later.iter().all(|(step, _)| {
                    matches!(step.kind, ExpressionKind::RepeatedSequence(_))
                });
                let alike = later.iter().all(|(_, count)| count == first);
                if later.is_empty() || list || alike {
                    continue;
                }
                gatherings.push(Gathering {
                    rule: RuleId(position),
                    held: rule_id,
                    sequence: expression.span,
                    places: places.iter().map(|(step, _)| step.span).collect(),
                });
            }
        });
    }
    gatherings
}

/// Returns the rules whose types `shape` holds.
pub(super) fn held(shape: &Shape) -> Vec<RuleId> {
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
        Shape::Converted(_) => Vec::new(),
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
        Shape::Enum(_) | Shape::Converted(_) => held(shape),
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
                    if let Variant::Rule { rule, boxed, .. } = variant {
                        *boxed = contains_this(*rule);
                    }
                }
            }
            Shape::Converted(_) => {}
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

/// Returns which rules get a type: the start rules and the rules their types
/// hold.
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
                                let held = match field.label {
                                    Some(label) => format!(
                                        "{}({})",
                                        name(label),
                                        name(field.rule)
                                    ),
                                    None => name(field.rule),
                                };
                                format!("{held} {:?}{boxed}", field.count)
                            })
                            .collect();
                        format!("struct {{{}}}", fields.join(", "))
                    }
                    Shape::Enum(variants) => {
                        let variants: Vec<String> = variants
                            .into_iter()
                            .map(|variant| match variant {
                                Variant::Rule { rule, named, boxed } => {
                                    let boxed =
                                        if boxed { " boxed" } else { "" };
                                    if named == rule {
                                        format!("{}{boxed}", name(rule))
                                    } else {
                                        format!(
                                            "{}({}){boxed}",
                                            name(named),
                                            name(rule)
                                        )
                                    }
                                }
                                Variant::Word(word) => format!("{word:?}"),
                            })
                            .collect();
                        format!("enum {{{}}}", variants.join(", "))
                    }
                    Shape::Converted(ty) => format!("converted {ty}"),
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
    fn makes_an_enum_of_a_group_with_parts_that_hold_nothing() {
        let rules = "a = 'a'; b = 'b'; (* hide *) ws = {' '};";
        assert_eq!(
            shapes_of(&format!("s = (a | b), ws, ';'; {rules}"))[0],
            shape("s", "enum {a, b}")
        );
        assert_eq!(
            shapes_of(&format!("s = (a | 'c'), ws; {rules}"))[0],
            shape("s", "struct {a Optional}")
        );
        assert_eq!(
            shapes_of(&format!("s = (a | b), a, ws; {rules}"))[0],
            shape("s", "struct {a Many, b Optional}")
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
    fn gives_a_rule_that_holds_an_inline_rule_what_it_holds() {
        assert_eq!(
            shapes_of(
                "s = g, g; (* inline *) g = '(', e, ')'; e = 'x' | f; \
                 f = 'y';"
            ),
            [
                shape("s", "struct {g(e) Fixed(2)}"),
                ("g".to_owned(), None),
                shape("e", "enum {\"x\", f}"),
                shape("f", "struct {}"),
            ]
        );
        assert_eq!(
            shapes_of("s = g | h; (* inline *) g = e, e; e = 'x'; h = 'y';")
                [0],
            shape("s", "struct {g(e) Many, h Optional}")
        );
        // Several rules have no one label, so their fields are held as is.
        assert_eq!(
            shapes_of("s = g, e; (* inline *) g = e, f; e = 'x'; f = 'y';")[0],
            shape("s", "struct {e Fixed(2), f One}")
        );
    }

    #[test]
    fn names_a_field_after_the_inline_rule_that_labels_it() {
        assert_eq!(
            shapes_of(
                "function = name, '(', [parameter, {',', parameter}], ')'; \
                 (* inline *) name = identifier; \
                 (* inline *) parameter = identifier; \
                 identifier = ? /[a-z]+/ ?;"
            )[0],
            shape(
                "function",
                "struct {name(identifier) One, \
                 parameter(identifier) Many}"
            )
        );
        // A label that holds another passes it on.
        assert_eq!(
            shapes_of(
                "s = outer, outer; (* inline *) outer = inner; \
                 (* inline *) inner = e; e = 'x';"
            )[0],
            shape("s", "struct {inner(e) Fixed(2)}")
        );
    }

    #[test]
    fn makes_a_converted_rule_its_rust_type() {
        assert_eq!(
            shapes_of(
                "s = n, [w]; (* as u8 *) n = d, {d}; d = '1'; \
                 (* as String *) w = 'x';"
            ),
            [
                shape("s", "struct {n One, w Optional}"),
                shape("n", "converted u8"),
                ("d".to_owned(), None),
                shape("w", "converted String"),
            ]
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
