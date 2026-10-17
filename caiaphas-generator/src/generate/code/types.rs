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

/// Returns the type a field or variant holds for the rule `rule`: its
/// conversion, or its own type as is or in a box.
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

/// Returns the derive attribute of a type, which is `textual` when it has
/// `text()`. One without compares by what it holds, having no text to compare
/// by.
fn derives(textual: bool) -> TokenStream {
    let compared = (!textual).then(|| quote! { ::core::cmp::PartialEq, });
    quote! {
        #[derive(
            ::core::fmt::Debug,
            ::core::clone::Clone,
            #compared
        )]
    }
}

/// Returns the code that reads `field` from `node`, inside the matches of its
/// label if it has one. A converted rule is read from its text.
fn read(names: &Names, field: &Field) -> TokenStream {
    let RuleId(held) = field.rule;
    let index = Literal::usize_unsuffixed(held);
    let node = field.label.map_or_else(
        || quote! { node },
        |RuleId(label)| {
            let label = Literal::usize_unsuffixed(label);
            quote! { node.inside(#label) }
        },
    );
    let Some(value) = &names.values[held] else {
        return match field.count {
            Count::One => quote! { #node.one(#index) },
            Count::Optional => quote! { #node.optional(#index) },
            Count::Fixed(_) => quote! { #node.array(#index) },
            Count::Many => quote! { #node.many(#index) },
        };
    };
    let converted = quote! { __caiaphas::Converted<#value> };
    match field.count {
        Count::One => quote! { #node.one::<#converted>(#index).0 },
        Count::Optional => quote! {
            #node.optional::<#converted>(#index).map(|value| value.0)
        },
        Count::Fixed(times) => {
            let length = Literal::usize_unsuffixed(usize::from(times));
            quote! {
                #node.array::<#converted, #length>(#index).map(|value| value.0)
            }
        }
        Count::Many => quote! {
            #node.many::<#converted>(#index)
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
    let traits = text_traits(name);
    let field_names: Vec<&Ident> = fields
        .iter()
        .map(|field| &names.fields[field.label.unwrap_or(field.rule).0])
        .collect();
    let mut types = Vec::new();
    let mut documentations = Vec::new();
    let mut builds = Vec::new();
    for field in fields {
        let RuleId(naming) = field.label.unwrap_or(field.rule);
        let held_name = name::spaced(&grammar.syntax_rules[naming]);
        let inner = held_type(names, field.rule, field.boxed);
        let (ty, documentation, build) = match field.count {
            Count::One => (
                inner,
                format!(" The `{held_name}` this `{this}` matched."),
                read(names, field),
            ),
            Count::Optional => (
                quote! { ::core::option::Option<#inner> },
                format!(
                    " The `{held_name}` this `{this}` matched, if it matched \
                     one."
                ),
                read(names, field),
            ),
            Count::Fixed(times) => {
                let length = Literal::usize_unsuffixed(usize::from(times));
                (
                    quote! { [#inner; #length] },
                    format!(
                        " The {times} `{held_name}` matches of this \
                         `{this}`, in order."
                    ),
                    read(names, field),
                )
            }
            Count::Many => (
                quote! { ::std::vec::Vec<#inner> },
                format!(
                    " Every `{held_name}` this `{this}` matched, in order."
                ),
                read(names, field),
            ),
        };
        types.push(ty);
        documentations.push(documentation);
        builds.push(build);
    }
    let derives = derives(true);
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

        #traits

        impl __caiaphas::FromNode for #name {
            fn from_node<W: __caiaphas::Width>(
                node: __caiaphas::Node<'_, W>,
            ) -> Self {
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
    parse: &TokenStream,
    documentation: &str,
) -> TokenStream {
    let name = &names.types[position];
    let mut declarations = Vec::new();
    let mut texts = Some(Vec::new());
    let mut spans = Some(Vec::new());
    let mut by_rule = Vec::new();
    let mut by_text = Vec::new();
    for variant in variants {
        match variant {
            Variant::Rule { rule, named, boxed } => {
                let RuleId(held) = *rule;
                let RuleId(name_giver) = *named;
                let variant = &names.types[name_giver];
                let documentation = format!(
                    " Matched `{}`.",
                    name::spaced(&grammar.syntax_rules[name_giver])
                );
                let ty = held_type(names, *rule, *boxed);
                let index = Literal::usize_unsuffixed(held);
                declarations.push(quote! {
                    #[doc = #documentation]
                    #variant(#ty)
                });
                // A value has no text or span, unless it is a spanned one, so
                // neither does the enum, nor one that holds it.
                if !names.textual[held] {
                    texts = None;
                }
                if !names.spanning[held] {
                    spans = None;
                }
                if let Some(texts) = &mut texts {
                    texts.push(
                        quote! { Self::#variant(inner) => inner.text() },
                    );
                }
                if let Some(spans) = &mut spans {
                    spans.push(
                        quote! { Self::#variant(inner) => inner.span() },
                    );
                }
                // The node of a label holds the one rule it names.
                let label = (*named != *rule)
                    .then(|| Literal::usize_unsuffixed(name_giver));
                let converted = names.values[held]
                    .as_ref()
                    .map(|value| quote! { __caiaphas::Converted<#value> });
                let read = match (&converted, &label) {
                    (Some(converted), None) => quote! {
                        <#converted as __caiaphas::FromNode>
                            ::from_node(child).0
                    },
                    (None, None) => {
                        quote! { __caiaphas::FromNode::from_node(child) }
                    }
                    (Some(converted), Some(_)) => {
                        quote! { child.one::<#converted>(#index).0 }
                    }
                    (None, Some(_)) => quote! { child.one(#index) },
                };
                let index = label.unwrap_or(index);
                by_rule.push(quote! { #index => Self::#variant(#read) });
            }
            Variant::Terminal(terminal) => {
                let variant = name::terminal_variant(terminal)
                    .expect("a terminal variant has a name");
                let documentation = format!(" Matched `{terminal}`.");
                declarations.push(quote! {
                    #[doc = #documentation]
                    #variant
                });
                if let Some(texts) = &mut texts {
                    texts.push(quote! { Self::#variant => #terminal });
                }
                // A terminal string has no place of its own to give.
                spans = None;
                by_text.push(quote! { #terminal => Self::#variant });
            }
        }
    }
    let derives = derives(texts.is_some());
    let traits = texts.is_some().then(|| text_traits(name));
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
    let span = spans.map(|spans| {
        quote! {
            /// Returns where this rule matched in its input.
            pub fn span(&self) -> &::caiaphas::Span {
                match self {
                    #(#spans,)*
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

            #span
        }

        #traits

        impl __caiaphas::FromNode for #name {
            fn from_node<W: __caiaphas::Width>(
                node: __caiaphas::Node<'_, W>,
            ) -> Self {
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

/// Returns the implementations that write, compare and hash the type `name` by
/// the text its rule matched, so two matches of the same text are equal
/// wherever they are.
fn text_traits(name: &Ident) -> TokenStream {
    quote! {
        impl ::core::fmt::Display for #name {
            fn fmt(
                &self,
                f: &mut ::core::fmt::Formatter<'_>,
            ) -> ::core::fmt::Result {
                f.write_str(self.text())
            }
        }

        impl ::core::cmp::PartialEq for #name {
            fn eq(&self, other: &Self) -> bool {
                self.text() == other.text()
            }
        }

        impl ::core::cmp::Eq for #name {}

        impl ::core::hash::Hash for #name {
            fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) {
                ::core::hash::Hash::hash(self.text(), state);
            }
        }
    }
}
