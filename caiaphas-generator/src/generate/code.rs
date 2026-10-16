//! Code writes the items of a grammar's module: the function that matches each
//! rule, the type of each rule, and how each type is built from a match.
//!
//! Generated code names everything outside the module by its full path, so
//! that a rule named `String` or `Debug` cannot change what it means.

mod types;

use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{
    analysis::{Facts, general_rules},
    generate::{exact, matching, name, shape::Shape},
    ir::{Grammar, Rule, RuleId},
};

/// The type and field names of every rule.
struct Names {
    types: Vec<Ident>,
    fields: Vec<Ident>,
}

/// Returns the items of the module that parses `grammar`, whose source is
/// `source`, with a type for each rule that has a shape in `shapes`. The
/// parser matches `matched`, which is `grammar` or the same rules rearranged.
pub(crate) fn module(
    grammar: &Grammar,
    matched: &Grammar,
    shapes: &[Option<Shape>],
    source: &str,
) -> TokenStream {
    let written_names = grammar
        .syntax_rules
        .iter()
        .map(|rule| rule.meta_identifier.written());
    let names = Names {
        types: written_names.clone().map(name::type_name).collect(),
        fields: written_names.map(name::field_name).collect(),
    };
    let facts = Facts::new(matched);
    let general = general_rules(&facts);
    let functions = matching::functions(&facts, &general);
    let written = Facts::new(grammar);
    let types = shapes.iter().enumerate().filter_map(|(position, shape)| {
        let shape = shape.as_ref()?;
        let rule = &grammar.syntax_rules[position];
        let start = grammar.start_syntax_rules.contains(&RuleId(position));
        let parse =
            start.then(|| parse(rule, RuleId(position), general[position]));
        let from_str = start.then(|| from_str(&names.types[position]));
        let documentation = format!(
            " The type of the `{}` rule:\n\n```text\n{}\n```",
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
                parse.as_ref(),
                &documentation,
            ),
        };
        Some(quote! { #type_code #from_str })
    });
    quote! {
        use ::caiaphas::__private as __caiaphas;

        #functions

        #(#types)*
    }
}

/// Returns the `parse` function of the start rule `rule_id`, written `rule`.
/// With `general`, it tries every way through the rule.
fn parse(rule: &Rule, rule_id: RuleId, general: bool) -> TokenStream {
    let name = name::spaced(rule);
    let documentation = format!(
        " Returns the `{name}` that all of `input` matches.\n\n # Errors\n\n \
         Returns where `input` stops matching a `{name}`, and what could \
         have come there instead."
    );
    let parse = if general {
        let (ends, build) =
            (exact::ends_name(rule_id), exact::build_name(rule_id));
        quote! {
            __caiaphas::parse_by_ends(
                input,
                #ends::<false>,
                #build::<false>,
                #ends::<true>,
            )
        }
    } else {
        let function = matching::function_name(rule_id);
        quote! {
            __caiaphas::parse(input, #function::<false>, #function::<true>)
        }
    };
    quote! {
        #[doc = #documentation]
        pub fn parse(
            input: &str,
        ) -> ::core::result::Result<Self, ::caiaphas::Error> {
            #parse
                .map(|tree| {
                    <Self as __caiaphas::FromNode>::from_node(tree.root())
                })
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
/// start rule, as its `parse` does.
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
