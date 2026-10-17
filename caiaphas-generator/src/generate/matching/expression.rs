//! The code of each expression a rule's function matches.

use proc_macro2::{Literal, TokenStream};
use quote::quote;

use super::{FunctionWriter, function_name, sets::pattern};
use crate::{
    analysis::CharacterSet,
    generate::exact::ExactWriter,
    ir::{Expression, ExpressionKind},
};

impl FunctionWriter<'_> {
    /// Returns the code of `expression`.
    pub(super) fn expression(&self, expression: &Expression) -> TokenStream {
        let some = quote! { ::core::option::Option::Some };
        let none = quote! { ::core::option::Option::None };
        match &expression.kind {
            ExpressionKind::SingleDefinition(expressions) => {
                let steps =
                    expressions.iter().map(|step| self.expression(step));
                quote! {
                    (|| {
                        #(let position = #steps?;)*
                        #some(position)
                    })()
                }
            }
            ExpressionKind::DefinitionsList(expressions) => {
                let branches = self.branches(expressions);
                if let [branch] = branches.as_slice() {
                    return branch.clone();
                }
                // The one alternative that can match is then the longest.
                if self.facts.start_apart(expressions) {
                    // A parse that only matches stops at the one branch that
                    // matches. One that finds out why an input does not match
                    // tries the rest too, for what they note.
                    return quote! {
                        {
                            let mut found: ::core::option::Option<usize> =
                                #none;
                            #(
                                if REPORTS || found.is_none() {
                                    let start = parser.mark();
                                    match #branches {
                                        #some(end) if found.is_none() => {
                                            found = #some(end);
                                        }
                                        _ => parser.truncate(start),
                                    }
                                }
                            )*
                            found
                        }
                    };
                }
                quote! {
                    {
                        let mark = parser.mark();
                        let mut longest: ::core::option::Option<usize> = #none;
                        #(
                            let start = parser.mark();
                            match #branches {
                                #some(end)
                                    if longest.is_none_or(|most| end > most) =>
                                {
                                    parser.discard(mark, start);
                                    longest = #some(end);
                                }
                                _ => parser.truncate(start),
                            }
                        )*
                        longest
                    }
                }
            }
            ExpressionKind::OptionalSequence(expression) => {
                if let Some(single) = self.single(expression) {
                    return quote! { #some(#single.unwrap_or(position)) };
                }
                let branch = self.branch(expression);
                quote! {
                    {
                        let mark = parser.mark();
                        match #branch {
                            #some(end) => #some(end),
                            #none => {
                                parser.truncate(mark);
                                #some(position)
                            }
                        }
                    }
                }
            }
            ExpressionKind::RepeatedSequence(expression) => {
                if let Some((pattern, expected)) = self.set_parts(expression) {
                    return quote! {
                        #some(parser.span(
                            position,
                            |c| ::core::matches!(c, #pattern),
                            &[#(#expected),*],
                        ))
                    };
                }
                let branch = self.branch(expression);
                quote! {
                    {
                        let mut end = position;
                        loop {
                            let position = end;
                            let mark = parser.mark();
                            // A round that reads nothing ends the repetition,
                            // since repeating it adds nothing.
                            match #branch {
                                #some(next) if next > end => end = next,
                                _ => {
                                    parser.truncate(mark);
                                    break #some(end);
                                }
                            }
                        }
                    }
                }
            }
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => {
                let integer = Literal::u32_unsuffixed(*integer);
                let primary = self.expression(syntactic_primary);
                quote! {
                    (|| {
                        let mut position = position;
                        for _ in 0..#integer {
                            position = #primary?;
                        }
                        #some(position)
                    })()
                }
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => {
                if let Some(code) =
                    self.set_except(syntactic_factor, syntactic_exception)
                {
                    return code;
                }
                let factor = self.expression(syntactic_factor);
                let exception = self.expression(syntactic_exception);
                quote! {
                    (|| {
                        let end = #factor?;
                        let mut within = parser.within(end);
                        let excluded = {
                            let parser = &mut within;
                            #exception
                        } == #some(end);
                        if excluded {
                            parser.excluded(&RULE, position, end);
                            #none
                        } else {
                            #some(end)
                        }
                    })()
                }
            }
            ExpressionKind::MetaIdentifier(rule_id) => {
                let function = function_name(*rule_id);
                quote! {
                    {
                        let found = #function(parser, position);
                        parser.matched(found)
                    }
                }
            }
            ExpressionKind::TerminalString(text) => {
                quote! { parser.text(position, #text) }
            }
            ExpressionKind::SpecialSequence(range) => {
                let (first, last) = (range.start(), range.end());
                quote! { parser.range(position, #first, #last) }
            }
            ExpressionKind::Not(ahead) => self.not(ahead),
            ExpressionKind::EmptySequence => quote! { #some(position) },
        }
    }

    /// Returns the code of a `not` of `ahead`, which matches nothing where
    /// `ahead` matches none of what comes next. A `not` of every character
    /// matches where the input ends, and one of a set of characters reads the
    /// next one. Any other tries every way through `ahead`, on a parser that
    /// notes nothing, since it only tells whether `ahead` matches.
    pub(in crate::generate) fn not(&self, ahead: &Expression) -> TokenStream {
        if let Some((characters, _)) = self.expectations(ahead) {
            let any = CharacterSet::from_range('\0'..=char::MAX);
            if any.is_subset(&characters) {
                return quote! { parser.end(position) };
            }
            let found = pattern(&characters).map_or_else(
                || quote! { false },
                |pattern| {
                    quote! {
                        ::core::matches!(
                            parser.character(position),
                            ::core::option::Option::Some(#pattern)
                        )
                    }
                },
            );
            return quote! {
                {
                    let found = #found;
                    parser.not(position, found)
                }
            };
        }
        // What a `not` looks for reads no rule, so no rule is general.
        let exact = ExactWriter {
            writer: *self,
            general: &[],
            settled: &[],
            recursive: &[],
        };
        let ends = exact.ends(ahead);
        quote! {
            {
                let found = {
                    let mut ahead = parser.ahead();
                    let parser = &mut ahead;
                    #ends
                }
                .first()
                .is_some();
                parser.not(position, found)
            }
        }
    }
}
