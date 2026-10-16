//! Code writes the items of a grammar's module: the tables its parser reads,
//! the type of each rule, and how each type is built from a match.
//!
//! Generated code names everything outside the module by its full path, so
//! that a rule named `String` or `Debug` cannot change what it means.

use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;

use crate::{
    generate::{
        name,
        shape::{Count, Field, Shape, Variant},
    },
    ir::{Expression, ExpressionKind, Grammar, Rule, RuleId},
};

/// The type and field names of every rule.
struct Names {
    types: Vec<Ident>,
    fields: Vec<Ident>,
}

/// Returns the items of the module that parses `grammar`, whose source is
/// `source`, with a type for each rule that has a shape in `shapes`.
pub(crate) fn module(
    grammar: &Grammar,
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
    let rules = grammar.syntax_rules.iter().map(|rule| {
        let name = written(rule);
        let expression = expression(&rule.expression);
        let hidden = rule.hidden;
        quote! {
            __caiaphas::Rule {
                name: #name,
                expression: #expression,
                hidden: #hidden,
            }
        }
    });
    let types = shapes.iter().enumerate().filter_map(|(position, shape)| {
        let shape = shape.as_ref()?;
        let rule = &grammar.syntax_rules[position];
        let parse = grammar
            .start_syntax_rules
            .contains(&RuleId(position))
            .then(|| parse(rule, position));
        let documentation = format!(
            " The type of the `{}` rule:\n\n```text\n{}\n```",
            written(rule),
            &source[rule.span.range()],
        );
        Some(match shape {
            Shape::Struct(fields) => structure(
                grammar,
                &names,
                position,
                fields,
                parse,
                &documentation,
            ),
            Shape::Enum(variants) => enumeration(
                grammar,
                &names,
                position,
                variants,
                parse,
                &documentation,
            ),
        })
    });
    quote! {
        use ::caiaphas::__private as __caiaphas;

        static __GRAMMAR: __caiaphas::Grammar =
            __caiaphas::Grammar { rules: &[#(#rules),*] };

        #(#types)*
    }
}

/// Returns the name of `rule` as written, with single spaces between its
/// words.
fn written(rule: &Rule) -> String {
    rule.meta_identifier.words().collect::<Vec<_>>().join(" ")
}

/// Returns the table of `expression`.
fn expression(expression: &Expression) -> TokenStream {
    let list = |expressions: &[Expression]| {
        let expressions = expressions.iter().map(self::expression);
        quote! { &[#(#expressions),*] }
    };
    match &expression.kind {
        ExpressionKind::SingleDefinition(expressions) => {
            let expressions = list(expressions);
            quote! { __caiaphas::Expression::Sequence(#expressions) }
        }
        ExpressionKind::DefinitionsList(expressions) => {
            let expressions = list(expressions);
            quote! { __caiaphas::Expression::Choice(#expressions) }
        }
        ExpressionKind::OptionalSequence(expression) => {
            let expression = self::expression(expression);
            quote! { __caiaphas::Expression::Optional(&#expression) }
        }
        ExpressionKind::RepeatedSequence(expression) => {
            let expression = self::expression(expression);
            quote! { __caiaphas::Expression::Repeated(&#expression) }
        }
        ExpressionKind::SyntacticFactor {
            integer,
            syntactic_primary,
        } => {
            let integer = Literal::u32_unsuffixed(*integer);
            let expression = self::expression(syntactic_primary);
            quote! { __caiaphas::Expression::Times(#integer, &#expression) }
        }
        ExpressionKind::SyntacticTerm {
            syntactic_factor,
            syntactic_exception,
        } => {
            let factor = self::expression(syntactic_factor);
            let exception = self::expression(syntactic_exception);
            quote! { __caiaphas::Expression::Except(&#factor, &#exception) }
        }
        ExpressionKind::MetaIdentifier(RuleId(position)) => {
            let position = Literal::usize_unsuffixed(*position);
            quote! { __caiaphas::Expression::Rule(#position) }
        }
        ExpressionKind::TerminalString(text) => {
            quote! { __caiaphas::Expression::Text(#text) }
        }
        ExpressionKind::SpecialSequence(range) => {
            let (first, last) = (range.start(), range.end());
            quote! { __caiaphas::Expression::Range(#first, #last) }
        }
        ExpressionKind::EmptySequence => {
            quote! { __caiaphas::Expression::Empty }
        }
    }
}

/// Returns the `parse` function of the start rule `rule`, numbered `position`.
fn parse(rule: &Rule, position: usize) -> TokenStream {
    let documentation = format!(
        " Returns the `{}` that all of `input` matches, or where `input` \
         stops matching it.",
        written(rule)
    );
    let position = Literal::usize_unsuffixed(position);
    quote! {
        #[doc = #documentation]
        pub fn parse(
            input: &str,
        ) -> ::core::result::Result<Self, ::caiaphas::Error> {
            __caiaphas::parse(&__GRAMMAR, #position, input).map(|tree| {
                <Self as __caiaphas::FromNode>::from_node(tree.root())
            })
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
    parse: Option<TokenStream>,
    documentation: &str,
) -> TokenStream {
    let name = &names.types[position];
    let this = written(&grammar.syntax_rules[position]);
    let field_names: Vec<&Ident> = fields
        .iter()
        .map(|field| &names.fields[field.rule.0])
        .collect();
    let mut types = Vec::new();
    let mut documentations = Vec::new();
    let mut builds = Vec::new();
    for field in fields {
        let RuleId(held) = field.rule;
        let held_name = written(&grammar.syntax_rules[held]);
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
            #parse

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
    parse: Option<TokenStream>,
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
                    written(&grammar.syntax_rules[held])
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
