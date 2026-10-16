//! Code writes the items of a grammar's module: the function that matches each
//! rule, the type of each rule, and how each type is built from a match.
//!
//! Generated code names everything outside the module by its full path, so
//! that a rule named `String` or `Debug` cannot change what it means.

use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;

use crate::{
    analysis::{Facts, general_rules},
    generate::{
        exact, matching, name,
        shape::{Count, Field, Shape, Variant},
    },
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
                structure(
                    grammar,
                    &names,
                    position,
                    fields,
                    &quote! { #parse #character },
                    &documentation,
                )
            }
            Shape::Enum(variants) => enumeration(
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

/// Returns the type of the rule `rule`, either as is or in a box.
fn held_type(names: &Names, RuleId(rule): RuleId, boxed: bool) -> TokenStream {
    let name = &names.types[rule];
    if boxed {
        quote! { ::std::boxed::Box<#name> }
    } else {
        quote! { #name }
    }
}

/// Returns the struct of the rule numbered `position`, with its `fields`.
fn structure(
    grammar: &Grammar,
    names: &Names,
    position: usize,
    fields: &[Field],
    methods: &TokenStream,
    documentation: &str,
) -> TokenStream {
    let name = &names.types[position];
    let this = name::spaced(&grammar.syntax_rules[position]);
    let field_names: Vec<&Ident> = fields
        .iter()
        .map(|field| &names.fields[field.rule.0])
        .collect();
    let mut types = Vec::new();
    let mut documentations = Vec::new();
    let mut builds = Vec::new();
    for field in fields {
        let RuleId(held) = field.rule;
        let held_name = name::spaced(&grammar.syntax_rules[held]);
        let inner = held_type(names, field.rule, field.boxed);
        let index = Literal::usize_unsuffixed(held);
        let (ty, documentation, build) = match field.count {
            Count::One => (
                inner,
                format!(" The `{held_name}` this `{this}` matched."),
                quote! { node.one(#index) },
            ),
            Count::Optional => (
                quote! { ::core::option::Option<#inner> },
                format!(
                    " The `{held_name}` this `{this}` matched, if it matched \
                     one."
                ),
                quote! { node.optional(#index) },
            ),
            Count::Fixed(times) => {
                let length = Literal::usize_unsuffixed(usize::from(times));
                (
                    quote! { [#inner; #length] },
                    format!(
                        " The {times} `{held_name}` matches of this \
                         `{this}`, in order."
                    ),
                    quote! { node.array(#index) },
                )
            }
            Count::Many => (
                quote! { ::std::vec::Vec<#inner> },
                format!(
                    " Every `{held_name}` this `{this}` matched, in order."
                ),
                quote! { node.many(#index) },
            ),
        };
        types.push(ty);
        documentations.push(documentation);
        builds.push(build);
    }
    quote! {
        #[doc = #documentation]
        #[derive(
            ::core::fmt::Debug,
            ::core::clone::Clone,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            ::core::hash::Hash,
        )]
        pub struct #name {
            #(
                #[doc = #documentations]
                pub #field_names: #types,
            )*
            span: ::caiaphas::Span,
        }

        impl #name {
            #methods

            /// Returns the text this rule matched.
            pub fn text(&self) -> &str {
                self.span.as_str()
            }

            /// Returns where this rule matched in its input.
            pub fn span(&self) -> &::caiaphas::Span {
                &self.span
            }
        }

        impl __caiaphas::FromNode for #name {
            fn from_node(node: __caiaphas::Node<'_>) -> Self {
                Self {
                    #(#field_names: #builds,)*
                    span: node.span(),
                }
            }
        }
    }
}

/// Returns the enum of the rule numbered `position`, with its `variants`.
fn enumeration(
    grammar: &Grammar,
    names: &Names,
    position: usize,
    variants: &[Variant],
    parse: Option<&TokenStream>,
    documentation: &str,
) -> TokenStream {
    let name = &names.types[position];
    let mut declarations = Vec::new();
    let mut texts = Vec::new();
    let mut by_rule = Vec::new();
    let mut by_text = Vec::new();
    for variant in variants {
        match variant {
            Variant::Rule { rule, boxed } => {
                let RuleId(held) = *rule;
                let variant = &names.types[held];
                let documentation = format!(
                    " Matched `{}`.",
                    name::spaced(&grammar.syntax_rules[held])
                );
                let ty = held_type(names, *rule, *boxed);
                let index = Literal::usize_unsuffixed(held);
                declarations.push(quote! {
                    #[doc = #documentation]
                    #variant(#ty)
                });
                texts.push(quote! { Self::#variant(inner) => inner.text() });
                by_rule.push(quote! {
                    #index => Self::#variant(
                        __caiaphas::FromNode::from_node(child),
                    )
                });
            }
            Variant::Word(word) => {
                let variant = name::word_variant(word)
                    .expect("a word variant is named after its word");
                let documentation = format!(" Matched `{word}`.");
                declarations.push(quote! {
                    #[doc = #documentation]
                    #variant
                });
                texts.push(quote! { Self::#variant => #word });
                by_text.push(quote! { #word => Self::#variant });
            }
        }
    }
    quote! {
        #[doc = #documentation]
        #[derive(
            ::core::fmt::Debug,
            ::core::clone::Clone,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            ::core::hash::Hash,
        )]
        pub enum #name {
            #(#declarations,)*
        }

        impl #name {
            #parse

            /// Returns the text this rule matched.
            pub fn text(&self) -> &str {
                match self {
                    #(#texts,)*
                }
            }
        }

        impl __caiaphas::FromNode for #name {
            fn from_node(node: __caiaphas::Node<'_>) -> Self {
                match node.child() {
                    ::core::option::Option::Some(child) => match child.rule() {
                        #(#by_rule,)*
                        _ => ::core::unreachable!(),
                    },
                    ::core::option::Option::None => match node.text() {
                        #(#by_text,)*
                        _ => ::core::unreachable!(),
                    },
                }
            }
        }
    }
}
