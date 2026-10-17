//! Outlines show the types a grammar's module holds as Rust declarations, each
//! after the rule it comes from, so a change to a grammar shows what it does
//! to the types without building anything.

use std::fmt::Write;

use super::Names;
use crate::{
    generate::{
        name,
        shape::{Count, Field, Shape, Variant},
    },
    ir::{Grammar, RuleId},
};

/// Returns the declaration of each type a rule of `grammar` gets, as `shapes`
/// tells, after the rule as `source` writes it.
pub(crate) fn outline(
    grammar: &Grammar,
    shapes: &[Option<Shape>],
    source: &str,
) -> String {
    let names = Names::new(grammar, shapes);
    let mut outline = String::new();
    for (position, shape) in shapes.iter().enumerate() {
        let (Some(Shape::Struct(_) | Shape::Enum(_)), Some(rule)) =
            (shape, grammar.syntax_rules.get(position))
        else {
            continue;
        };
        if !outline.is_empty() {
            outline.push('\n');
        }
        for line in source[rule.span.range()].lines() {
            let _ = writeln!(outline, "/// {}", line.trim_end());
        }
        let name = &names.types[position];
        match shape {
            Some(Shape::Struct(fields)) if fields.is_empty() => {
                let _ = writeln!(outline, "pub struct {name} {{}}");
            }
            Some(Shape::Struct(fields)) => {
                let _ = writeln!(outline, "pub struct {name} {{");
                for field in fields {
                    let RuleId(held) = field.label.unwrap_or(field.rule);
                    let _ = writeln!(
                        outline,
                        "    pub {}: {},",
                        names.fields[held],
                        field_type(grammar, &names, field)
                    );
                }
                outline.push_str("}\n");
            }
            Some(Shape::Enum(variants)) => {
                let _ = writeln!(outline, "pub enum {name} {{");
                for variant in variants {
                    let _ = writeln!(
                        outline,
                        "    {},",
                        variant_declaration(grammar, &names, variant)
                    );
                }
                outline.push_str("}\n");
            }
            _ => {}
        }
    }
    outline
}

/// Returns the type a field or variant holds for the rule `rule`, in a box
/// when `boxed`: its conversion as written, or its own type.
fn held(
    grammar: &Grammar,
    names: &Names,
    rule: RuleId,
    boxed: bool,
) -> String {
    let RuleId(position) = rule;
    if let Some(value) = &grammar.syntax_rules[position].converts_to {
        return value.clone();
    }
    let name = &names.types[position];
    if boxed {
        format!("Box<{name}>")
    } else {
        name.to_string()
    }
}

/// Returns the type of `field`, as many times as its rule is held.
fn field_type(grammar: &Grammar, names: &Names, field: &Field) -> String {
    let inner = held(grammar, names, field.rule, field.boxed);
    match field.count {
        Count::One => inner,
        Count::Optional => format!("Option<{inner}>"),
        Count::Fixed(times) => format!("[{inner}; {times}]"),
        Count::Many => format!("Vec<{inner}>"),
    }
}

/// Returns how `variant` is declared.
fn variant_declaration(
    grammar: &Grammar,
    names: &Names,
    variant: &Variant,
) -> String {
    match variant {
        Variant::Rule { rule, named, boxed } => {
            let RuleId(name_giver) = *named;
            format!(
                "{}({})",
                names.types[name_giver],
                held(grammar, names, *rule, *boxed)
            )
        }
        Variant::Word(word) => name::word_variant(word)
            .expect("a word variant is named after its word")
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{dictionary::tests::build_valid, generate::shape};

    fn outline(source: &str) -> String {
        let grammar = build_valid(source);
        super::outline(&grammar, &shape::shapes(&grammar), source)
    }

    #[test]
    fn declares_each_type_after_its_rule() {
        let source = "shape = circle | square | 'none';\n\
                      circle = 'circle ', size;\n\
                      square = 'square ', size, 'x', size, [label];\n\
                      label = ' ', {? U+0061..U+007A ?};\n\
                      (* as u32 *) size = digit, {digit};\n\
                      (* hide *) digit = ? U+0030..U+0039 ?;";
        assert_eq!(
            outline(source),
            "/// shape = circle | square | 'none';\n\
             pub enum Shape {\n    Circle(Circle),\n    Square(Square),\n    \
             None,\n}\n\
             \n\
             /// circle = 'circle ', size;\n\
             pub struct Circle {\n    pub size: u32,\n}\n\
             \n\
             /// square = 'square ', size, 'x', size, [label];\n\
             pub struct Square {\n    pub size: [u32; 2],\n    \
             pub label: Option<Label>,\n}\n\
             \n\
             /// label = ' ', {? U+0061..U+007A ?};\n\
             pub struct Label {}\n"
        );
    }

    #[test]
    fn boxes_a_type_that_could_contain_itself() {
        let found = outline("list = '(', [list], ')';");
        assert!(found.contains("pub list: Option<Box<List>>,"), "{found}");
    }
}
