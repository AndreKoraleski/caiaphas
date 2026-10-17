//! Code writes the items of a grammar's module: the function that matches each
//! rule, the type of each rule, and how each type is built from a match.
//!
//! Generated code names everything outside the module by its full path, so
//! that a rule named `String` or `Debug` cannot change what it means.

mod outline;
mod types;

pub(crate) use outline::outline;

use std::fmt::Write;

use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{
    analysis::{Facts, general_rules},
    generate::{
        exact, matching, name,
        shape::{self, Shape, Variant},
    },
    ir::{Grammar, Rule, RuleId},
};

/// What writing the type of every rule needs to know.
struct Names {
    types: Vec<Ident>,
    fields: Vec<Ident>,
    /// The Rust type a conversion makes each rule's type, if any.
    values: Vec<Option<TokenStream>>,
    /// Whether each rule's type has `text()`.
    textual: Vec<bool>,
    /// Whether each rule's type has `span()`.
    spanning: Vec<bool>,
}

impl Names {
    /// Returns the names of the types and fields of the rules of `grammar`,
    /// whose types `shapes` tells.
    fn new(grammar: &Grammar, shapes: &[Option<Shape>]) -> Self {
        let written_names = grammar
            .syntax_rules
            .iter()
            .map(|rule| rule.meta_identifier.written());
        Self {
            types: written_names.clone().map(name::type_name).collect(),
            fields: written_names.map(name::field_name).collect(),
            values: grammar.syntax_rules.iter().map(value_type).collect(),
            textual: textual(shapes),
            spanning: spanning(shapes),
        }
    }
}

/// Returns the items of the module that parses `grammar`, with a type for each
/// rule that has a shape in `shapes`. The parser matches the same rules
/// rearranged, as `matched` holds them. Each type's docs quote its rule from
/// `source`. Its parsers follow `depth` matches of recursive rules inside one
/// another, or as many as the runtime does by default.
pub(crate) fn module(
    grammar: &Grammar,
    matched: &Grammar,
    shapes: &[Option<Shape>],
    source: &str,
    depth: Option<usize>,
) -> TokenStream {
    let names = Names::new(grammar, shapes);
    let rule_names = grammar.syntax_rules.iter().map(name::spaced);
    let facts = Facts::new(matched);
    let general = general_rules(&facts);
    // Only a rule whose node a type reads builds one. An inline rule's node
    // stands for its children, so it builds one too.
    let builds: Vec<bool> = grammar
        .syntax_rules
        .iter()
        .zip(shapes)
        .map(|(rule, shape)| !rule.hidden && (shape.is_some() || rule.inline))
        .collect();
    let functions = matching::functions(&facts, &general, &builds);
    let written = Facts::new(grammar);
    let types = shapes.iter().enumerate().filter_map(|(position, shape)| {
        let shape = shape.as_ref()?;
        let rule = &grammar.syntax_rules[position];
        // Every type parses, so a part of a grammar can be read and tested
        // alone.
        let parse = parse(rule, RuleId(position), general[position]);
        let from_str = from_str(&names.types[position]);
        let comments = rule.comments.iter().fold(
            String::new(),
            |mut comments, comment| {
                let _ = write!(comments, " {comment}\n\n");
                comments
            },
        );
        let documentation = format!(
            "{comments} The type of the `{}` rule.\n\n```text\n{}\n```",
            name::spaced(rule),
            &source[rule.span.range()],
        );
        let type_code = match shape {
            Shape::Struct(fields) => {
                let character = written
                    .single_characters(&rule.expression)
                    .is_some()
                    .then(character);
                types::structure(
                    grammar,
                    &names,
                    position,
                    fields,
                    &quote! { #parse #character },
                    &documentation,
                )
            }
            Shape::Enum(variants) => types::enumeration(
                grammar,
                &names,
                position,
                variants,
                &parse,
                &documentation,
            ),
            Shape::Converted(_) => return None,
        };
        Some(quote! { #type_code #from_str })
    });
    let depth = depth.map_or_else(
        || quote! { __caiaphas::DEPTH },
        |depth| {
            let depth = proc_macro2::Literal::usize_unsuffixed(depth);
            quote! { #depth }
        },
    );
    quote! {
        use ::caiaphas::__private as __caiaphas;
        // A conversion names its type as where the macro is called.
        #[allow(unused_imports, clippy::wildcard_imports)]
        use super::*;

        /// How many matches of recursive rules the parsers follow inside one
        /// another.
        const DEPTH: usize = #depth;

        /// The name of each rule, as errors and tree dumps give it.
        #[cfg(test)]
        const NAMES: &[&str] = &[#(#rule_names),*];

        #functions

        #(#types)*
    }
}

/// Returns the Rust type a conversion makes the type of `rule`, if any.
pub(super) fn value_type(rule: &Rule) -> Option<TokenStream> {
    let ty = rule.converts_to.as_ref()?;
    Some(ty.parse().expect("a conversion names a Rust type"))
}

/// Returns whether the type of each rule has `text()`. A converted rule's type
/// does unless it is a `Spanned` one, and an enum has it when every type it
/// holds has it.
fn textual(shapes: &[Option<Shape>]) -> Vec<bool> {
    through_enums(shapes, |_| true)
}

/// Returns whether the type of each rule has `span()`, as for `text()`, but an
/// enum with a variant for a word has none.
fn spanning(shapes: &[Option<Shape>]) -> Vec<bool> {
    through_enums(shapes, |variants| {
        variants
            .iter()
            .all(|variant| matches!(variant, Variant::Rule { .. }))
    })
}

/// Returns whether the type of each rule has a method that a struct has, a
/// converted type has when it is a `Spanned` one, and an enum has when
/// `variants` allows its own and every type it holds has the method.
fn through_enums(
    shapes: &[Option<Shape>],
    variants: impl Fn(&[Variant]) -> bool,
) -> Vec<bool> {
    let mut has: Vec<bool> = shapes
        .iter()
        .map(|shape| match shape {
            Some(Shape::Converted(ty)) => keeps_span(ty),
            Some(Shape::Enum(own)) => variants(own),
            _ => true,
        })
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for (position, shape) in shapes.iter().enumerate() {
            let Some(shape @ Shape::Enum(_)) = shape else {
                continue;
            };
            if has[position]
                && shape::held(shape).iter().any(|&RuleId(rule)| !has[rule])
            {
                has[position] = false;
                changed = true;
            }
        }
    }
    has
}

/// Returns whether the type `ty` a conversion names is a `Spanned` one, which
/// keeps the text it was read from and where.
pub(super) fn keeps_span(ty: &str) -> bool {
    let ty = ty.strip_prefix("::").unwrap_or(ty);
    ty.strip_prefix("caiaphas::Spanned<")
        .is_some_and(|rest| rest.ends_with('>'))
}

/// Returns the `parse` function of the rule `rule_id`, written `rule`. With
/// `general`, it tries every way through the rule.
fn parse(rule: &Rule, rule_id: RuleId, general: bool) -> TokenStream {
    let name = name::spaced(rule);
    let documentation = format!(
        " Returns the `{name}` that all of `input` matches.\n\n # Errors\n\n \
         Returns where `input` stops matching a `{name}`, and what could \
         have come there instead."
    );
    let at_documentation = format!(
        " Returns the `{name}` that all of `input` matches, where `input` \
         starts at `line` and `column` of a larger text, both counted from \
         1, such as a line of a log or a block in another file. The lines \
         and columns of its spans and error count from there, and offsets \
         and ranges stay those in `input`.\n\n # Errors\n\n Returns where \
         `input` stops matching a `{name}`, and what could have come there \
         instead."
    );
    let recovering_documentation = format!(
        " Returns the `{name}` that `input` matches once each error that a \
         rule written with `(* recover *)` can skip is skipped, if any, and \
         every error found."
    );
    let (root, explain) = if general {
        let (ends, build) =
            (exact::ends_name(rule_id), exact::build_name(rule_id));
        (
            quote! {
                let length = parser.length();
                if #ends::<false, W>(parser, 0).contains(length) {
                    #build::<false, W>(parser, 0, length)
                } else {
                    ::core::option::Option::None
                }
            },
            quote! { #ends::<true, u64>(parser, 0).iter().collect() },
        )
    } else {
        let function = matching::function_name(rule_id);
        (
            quote! {
                let found = #function::<false, W>(parser, 0);
                __caiaphas::whole(found, parser.length())
            },
            quote! {
                #function::<true, u64>(parser, 0)
                    .map(|(end, _)| end)
                    .into_iter()
                    .collect()
            },
        )
    };
    // The runtime compiles the rule for each width of node, so it names the
    // rule through a type.
    let start = quote! {
        struct Start;

        impl __caiaphas::Start for Start {
            fn root<W: __caiaphas::Width>(
                parser: &mut __caiaphas::Parser<'_, false, W>,
            ) -> ::core::option::Option<usize> {
                #root
            }

            fn explain(
                parser: &mut __caiaphas::Parser<'_, true, u64>,
            ) -> ::std::vec::Vec<usize> {
                #explain
            }
        }
    };
    quote! {
        #[doc = #documentation]
        pub fn parse(
            input: &str,
        ) -> ::core::result::Result<Self, ::caiaphas::Error> {
            #start
            __caiaphas::parse::<Start>(input, DEPTH).map(|tree| {
                tree.value::<Self>()
            })
        }

        #[doc = #at_documentation]
        pub fn parse_at(
            input: &str,
            line: usize,
            column: usize,
        ) -> ::core::result::Result<Self, ::caiaphas::Error> {
            #start
            __caiaphas::parse_at::<Start>(input, DEPTH, line, column).map(
                |tree| tree.value::<Self>(),
            )
        }

        #[doc = #recovering_documentation]
        pub fn parse_recovering(input: &str) -> ::caiaphas::Recovered<Self> {
            #start
            let (tree, errors) =
                __caiaphas::parse_recovering::<Start>(input, DEPTH);
            let value = tree.map(|tree| {
                tree.value::<Self>()
            });
            __caiaphas::recovered(value, errors)
        }

        // Tests compare what a grammar run without building anything finds
        // with this, which only they see.
        #[cfg(test)]
        #[doc(hidden)]
        pub fn __tree(
            input: &str,
        ) -> ::core::result::Result<::std::string::String, ::caiaphas::Error> {
            #start
            __caiaphas::parse::<Start>(input, DEPTH)
                .map(|tree| tree.dump(NAMES))
        }

        #[cfg(test)]
        #[doc(hidden)]
        pub fn __tree_recovering(
            input: &str,
        ) -> (
            ::core::option::Option<::std::string::String>,
            ::std::vec::Vec<::caiaphas::Error>,
        ) {
            #start
            let (tree, errors) =
                __caiaphas::parse_recovering::<Start>(input, DEPTH);
            (tree.map(|tree| tree.dump(NAMES)), errors)
        }
    }
}

/// Returns the `char` method of the type of a rule that only ever matches one
/// character.
fn character() -> TokenStream {
    quote! {
        /// Returns the one character this rule matched.
        pub fn char(&self) -> char {
            match self.span.as_str().chars().next() {
                ::core::option::Option::Some(character) => character,
                ::core::option::Option::None => ::core::unreachable!(),
            }
        }
    }
}

/// Returns the implementation that lets `str::parse` read the type `name` of a
/// rule, as its `parse` does.
fn from_str(name: &Ident) -> TokenStream {
    quote! {
        impl ::core::str::FromStr for #name {
            type Err = ::caiaphas::Error;

            fn from_str(
                input: &str,
            ) -> ::core::result::Result<Self, ::caiaphas::Error> {
                Self::parse(input)
            }
        }
    }
}
