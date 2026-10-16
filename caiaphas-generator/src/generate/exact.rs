//! Exact matching writes the two functions of each rule that tries every way
//! through it. One tells where the rule's matches at a position can end, in
//! the order the rule prefers them. The other builds the node of the match
//! from a position to one of those ends.
//!
//! Code that tells where an expression can end reads `parser` and `position`,
//! as in matching, and gives its ends.

mod build;

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

use crate::{
    generate::{
        code,
        matching::{self, FunctionWriter},
    },
    ir::{Expression, ExpressionKind, Rule, RuleId},
};

/// Returns the name of the function that tells where the matches of the rule
/// `rule_id` can end.
pub(crate) fn ends_name(RuleId(position): RuleId) -> Ident {
    format_ident!("ends_{position}")
}

/// Returns the name of the function that builds the node of a match of the
/// rule `rule_id`.
pub(crate) fn build_name(RuleId(position): RuleId) -> Ident {
    format_ident!("build_{position}")
}

/// Writes the functions of one rule that tries every way through it.
pub(super) struct ExactWriter<'a> {
    /// Writes the code of what only ever matches one character, and holds the
    /// facts and first match of the rule.
    pub(super) writer: FunctionWriter<'a>,
    /// Which rules try every way through them.
    pub(super) general: &'a [bool],
}

impl ExactWriter<'_> {
    /// Returns the functions of `rule`. A hidden rule has no node, so it only
    /// gets the one that tells where it can end.
    pub(super) fn functions(
        &self,
        rule_id: RuleId,
        rule: &Rule,
    ) -> TokenStream {
        let (ends, build) = (ends_name(rule_id), build_name(rule_id));
        let constant = self.writer.rule_constant(rule_id, rule, true);
        let body = self.ends(&rule.expression);
        // A rule whose type its text converts to keeps only the ends whose
        // text the type reads.
        let convert = code::value_type(rule).map(|value| {
            quote! {
                let mut ends = ends;
                ends.retain(|end| {
                    parser.accepts::<#value>(&RULE, position, end)
                });
            }
        });
        let ends_function = quote! {
            #[doc(hidden)]
            fn #ends<const REPORTS: bool>(
                parser: &mut __caiaphas::Parser<'_, REPORTS>,
                position: usize,
            ) -> __caiaphas::Ends {
                #constant
                if let ::core::option::Option::Some(ends) =
                    parser.recall_ends(&RULE, position)
                {
                    return ends;
                }
                let start = parser.start_rule(position);
                let ends = #body;
                #convert
                parser.finish_ends(&RULE, start, ends)
            }
        };
        if rule.hidden {
            return ends_function;
        }
        let building = self.build(&rule.expression);
        quote! {
            #ends_function

            #[doc(hidden)]
            fn #build<const REPORTS: bool>(
                parser: &mut __caiaphas::Parser<'_, REPORTS>,
                position: usize,
                end: usize,
            ) -> ::core::option::Option<usize> {
                #constant
                let start = parser.start_rule(position);
                #building
                parser.built(&RULE, start, end)
            }
        }
    }

    /// Returns the function of `rule` that a greedy rule reads. It takes the
    /// end `rule` prefers and builds the node of its match up to there.
    pub(super) fn committed(rule_id: RuleId, rule: &Rule) -> TokenStream {
        let function = matching::function_name(rule_id);
        let ends = ends_name(rule_id);
        let node = if rule.hidden {
            quote! { ::core::option::Option::None }
        } else {
            let build = build_name(rule_id);
            quote! { #build(parser, position, end) }
        };
        quote! {
            #[doc(hidden)]
            fn #function<const REPORTS: bool>(
                parser: &mut __caiaphas::Parser<'_, REPORTS>,
                position: usize,
            ) -> __caiaphas::Match {
                let end = #ends(parser, position).first()?;
                let node = #node;
                ::core::option::Option::Some((end, node))
            }
        }
    }

    /// Returns the code that tells where `expression` can end.
    ///
    /// The ends of a rule that takes the first match come in the order of the
    /// ways that end there, by the first branch each takes, then by what comes
    /// after. Those of any other rule come with the end of the most text
    /// first.
    fn ends(&self, expression: &Expression) -> TokenStream {
        let ends = quote! { __caiaphas::Ends };
        if let Some(single) = self.writer.single(expression) {
            return quote! { #ends::of(#single) };
        }
        let ways = match &expression.kind {
            ExpressionKind::SingleDefinition(steps) => {
                let steps = steps.iter().map(|step| self.part(step));
                quote! {
                    {
                        let ends = #ends::one(position);
                        #(let ends = parser.then(&ends, #steps);)*
                        ends
                    }
                }
            }
            ExpressionKind::DefinitionsList(alternatives) => {
                let alternatives = alternatives
                    .iter()
                    .map(|alternative| self.branch(alternative));
                quote! {
                    {
                        let mut ends = #ends::none();
                        #(ends.extend(#alternatives);)*
                        ends
                    }
                }
            }
            ExpressionKind::OptionalSequence(sequence) => {
                let sequence = self.branch(sequence);
                quote! {
                    {
                        let mut ends = #sequence;
                        ends.add(position);
                        ends
                    }
                }
            }
            ExpressionKind::RepeatedSequence(sequence) => {
                if let Some((pattern, expected)) =
                    self.writer.set_parts(sequence)
                {
                    return quote! {
                        parser.span_ends(
                            position,
                            |c| ::core::matches!(c, #pattern),
                            &[#(#expected),*],
                        )
                    };
                }
                let round = self.branch(sequence);
                quote! { parser.rounds(position, |parser, position| #round) }
            }
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => {
                let integer = Literal::u32_unsuffixed(*integer);
                let primary = self.part(syntactic_primary);
                quote! {
                    parser.times(#ends::one(position), #integer, #primary)
                }
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor,
                syntactic_exception,
            } => {
                if let Some(code) = self
                    .writer
                    .set_except(syntactic_factor, syntactic_exception)
                {
                    return quote! { #ends::of(#code) };
                }
                // The ends of the factor come in its order, and an exception
                // only takes some away.
                let factor = self.ends(syntactic_factor);
                let exception = self.ends(syntactic_exception);
                return quote! {
                    {
                        let mut ends = #factor;
                        ends.retain(|end| {
                            let excluded = {
                                let mut within = parser.within(end);
                                let parser = &mut within;
                                (#exception).contains(end)
                            };
                            if excluded {
                                parser.excluded(&RULE, position, end);
                            }
                            !excluded
                        });
                        ends
                    }
                };
            }
            ExpressionKind::MetaIdentifier(rule_id) => {
                let RuleId(position) = *rule_id;
                if self.general[position] {
                    let function = ends_name(*rule_id);
                    return quote! { #function(parser, position) };
                }
                let function = matching::function_name(*rule_id);
                return quote! {
                    #ends::of(#function(parser, position).map(|(end, _)| end))
                };
            }
            ExpressionKind::TerminalString(text) => {
                return quote! { #ends::of(parser.text(position, #text)) };
            }
            ExpressionKind::SpecialSequence(range) => {
                let (first, last) = (range.start(), range.end());
                return quote! {
                    #ends::of(parser.range(position, #first, #last))
                };
            }
            ExpressionKind::EmptySequence => {
                return quote! { #ends::one(position) };
            }
        };
        if self.writer.first_match {
            ways
        } else {
            quote! { #ways.longest_first() }
        }
    }

    /// Returns the closure that tells where `expression` can end from the
    /// position it is given.
    fn part(&self, expression: &Expression) -> TokenStream {
        if let ExpressionKind::EmptySequence = expression.kind {
            return quote! { |_, position| __caiaphas::Ends::one(position) };
        }
        let ends = self.ends(expression);
        quote! { |parser, position| #ends }
    }

    /// Returns the code that tells where `expression` can end as a branch. A
    /// parse that only matches skips the branch when the next character is
    /// none of those it can start with, as in matching.
    fn branch(&self, expression: &Expression) -> TokenStream {
        let code = self.ends(expression);
        let none = quote! { __caiaphas::Ends::none() };
        let single = self.writer.single(expression).is_some();
        self.writer.guarded(expression, code, &none, single)
    }
}
