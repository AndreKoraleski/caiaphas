//! Types writes the struct or enum of each rule, and how it is built from the
//! node of a match.

use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;

use super::Names;
use crate::{
    generate::{
        name,
        shape::{Count, Field, Variant},
    },
    ir::{Grammar, RuleId},
};

/// Returns the type of the rule `rule`, either as is or in a box.
fn held_type(names: &Names, RuleId(rule): RuleId, boxed: bool) -> TokenStream {
    if let Some(value) = &names.values[rule] {
        return value.clone();
    }
    let name = &names.types[rule];
    if boxed {
        quote! { ::std::boxed::Box<#name> }
    } else {
        quote! { #name }
    }
}

/// Returns the derive attribute of the type of the rule numbered `position`.
fn derives(names: &Names, position: usize) -> TokenStream {
    let hash = names.hashable[position]
        .then(|| quote! { ::core::cmp::Eq, ::core::hash::Hash, });
    quote! {
        #[derive(
            ::core::fmt::Debug,
            ::core::clone::Clone,
            ::core::cmp::PartialEq,
            #hash
        )]
    }
}

/// Returns the code that reads the field of `rule`, held `count` times, from
/// `node`. A converted rule is read from its text.
fn read(names: &Names, rule: RuleId, count: Count) -> TokenStream {
    let RuleId(held) = rule;
    let index = Literal::usize_unsuffixed(held);
    let Some(value) = &names.values[held] else {
        return match count {
            Count::One => quote! { node.one(#index) },
            Count::Optional => quote! { node.optional(#index) },
            Count::Fixed(_) => quote! { node.array(#index) },
            Count::Many => quote! { node.many(#index) },
        };
    };
    let converted = quote! { __caiaphas::Converted<#value> };
    match count {
        Count::One => quote! { node.one::<#converted>(#index).0 },
        Count::Optional => {
            quote! { node.optional::<#converted>(#index).map(|value| value.0) }
        }
        Count::Fixed(times) => {
            let length = Literal::usize_unsuffixed(usize::from(times));
            quote! {
                node.array::<#converted, #length>(#index).map(|value| value.0)
            }
        }
        Count::Many => quote! {
            node.many::<#converted>(#index)
                .into_iter()
                .map(|value| value.0)
                .collect()
        },
    }
}

/// Returns the struct of the rule numbered `position`, with its `fields`.
pub(super) fn structure(
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
        let (ty, documentation, build) = match field.count {
            Count::One => (
                inner,
                format!(" The `{held_name}` this `{this}` matched."),
                read(names, field.rule, field.count),
            ),
            Count::Optional => (
                quote! { ::core::option::Option<#inner> },
                format!(
                    " The `{held_name}` this `{this}` matched, if it matched \
                     one."
                ),
                read(names, field.rule, field.count),
            ),
            Count::Fixed(times) => {
                let length = Literal::usize_unsuffixed(usize::from(times));
                (
                    quote! { [#inner; #length] },
                    format!(
                        " The {times} `{held_name}` matches of this \
                         `{this}`, in order."
                    ),
                    read(names, field.rule, field.count),
                )
            }
            Count::Many => (
                quote! { ::std::vec::Vec<#inner> },
                format!(
                    " Every `{held_name}` this `{this}` matched, in order."
                ),
                read(names, field.rule, field.count),
            ),
        };
        types.push(ty);
        documentations.push(documentation);
        builds.push(build);
    }
    let derives = derives(names, position);
    quote! {
        #[doc = #documentation]
        #derives
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
pub(super) fn enumeration(
    grammar: &Grammar,
    names: &Names,
    position: usize,
    variants: &[Variant],
    parse: Option<&TokenStream>,
    documentation: &str,
) -> TokenStream {
    let name = &names.types[position];
    let mut declarations = Vec::new();
    let mut texts = Some(Vec::new());
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
                if let Some(value) = &names.values[held] {
                    // A value has no text, so neither does the enum.
                    texts = None;
                    by_rule.push(quote! {
                        #index => Self::#variant(
                            <__caiaphas::Converted<#value> as
                                __caiaphas::FromNode>::from_node(child).0,
                        )
                    });
                } else {
                    if let Some(texts) = &mut texts {
                        texts.push(
                            quote! { Self::#variant(inner) => inner.text() },
                        );
                    }
                    by_rule.push(quote! {
                        #index => Self::#variant(
                            __caiaphas::FromNode::from_node(child),
                        )
                    });
                }
            }
            Variant::Word(word) => {
                let variant = name::word_variant(word)
                    .expect("a word variant is named after its word");
                let documentation = format!(" Matched `{word}`.");
                declarations.push(quote! {
                    #[doc = #documentation]
                    #variant
                });
                if let Some(texts) = &mut texts {
                    texts.push(quote! { Self::#variant => #word });
                }
                by_text.push(quote! { #word => Self::#variant });
            }
        }
    }
    let derives = derives(names, position);
    let text = texts.map(|texts| {
        quote! {
            /// Returns the text this rule matched.
            pub fn text(&self) -> &str {
                match self {
                    #(#texts,)*
                }
            }
        }
    });
    quote! {
        #[doc = #documentation]
        #derives
        pub enum #name {
            #(#declarations,)*
        }

        impl #name {
            #parse

            #text
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
