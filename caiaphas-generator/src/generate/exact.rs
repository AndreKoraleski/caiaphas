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
    analysis::Facts,
    generate::{
        code,
        matching::{self, FunctionWriter},
    },
    ir::{Expression, ExpressionKind, Grammar, Rule, RuleId},
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
    /// Which rules keep the order a rule that takes the first match gives its
    /// ends, as [`ordered_rules`] tells.
    pub(super) ordered: &'a [bool],
    /// Which rules end at the same places however much input follows, as
    /// [`settled_rules`] tells.
    pub(super) settled: &'a [bool],
}

/// Returns which rules end at the same places however much input follows them:
/// those that always end at one place, and those that try every way and only
/// read rules like that. Others can take a way as it comes, and that can hang
/// on what follows.
pub(super) fn settled_rules(facts: &Facts<'_>, general: &[bool]) -> Vec<bool> {
    let rules = &facts.grammar().syntax_rules;
    let once: Vec<bool> = rules
        .iter()
        .map(|rule| facts.ends_once(&rule.expression))
        .collect();
    let mut settled: Vec<bool> = once
        .iter()
        .zip(general)
        .map(|(&once, &general)| once || general)
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for (position, rule) in rules.iter().enumerate() {
            if !settled[position] || once[position] {
                continue;
            }
            let mut unsettled = false;
            rule.expression
                .visit_meta_identifiers(&mut |RuleId(read), _| {
                    unsettled |= !settled[read];
                });
            if unsettled {
                settled[position] = false;
                changed = true;
            }
        }
    }
    settled
}

/// Returns which rules keep the order a rule that takes the first match gives
/// its ends: the rules that try every way and take the first match, and those
/// that read one of them. A rule takes its first match wherever it is read.
pub(super) fn ordered_rules(grammar: &Grammar, general: &[bool]) -> Vec<bool> {
    let rules = &grammar.syntax_rules;
    let mut ordered: Vec<bool> = rules
        .iter()
        .zip(general)
        .map(|(rule, &general)| general && rule.first_match)
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for (position, rule) in rules.iter().enumerate() {
            if ordered[position] || !general[position] {
                continue;
            }
            let mut reads = false;
            rule.expression
                .visit_meta_identifiers(&mut |RuleId(read), _| {
                    reads |= ordered[read];
                });
            if reads {
                ordered[position] = true;
                changed = true;
            }
        }
    }
    ordered
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
        // A rule that recovers goes on from where a parse that recovers tells
        // it to, when it fails, and builds its node from there.
        let (finish, start_from) = if rule.recovers {
            (
                quote! {
                    let ends = parser.finish_ends(&RULE, start, ends);
                    if ends.first().is_some() {
                        return ends;
                    }
                    parser.recover_ends(&RULE, start, #ends::<REPORTS>)
                },
                Some(quote! {
                    let position = parser.recovered(&RULE, position);
                }),
            )
        } else {
            (quote! { parser.finish_ends(&RULE, start, ends) }, None)
        };
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
                #finish
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
                #start_from
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
    /// first, unless they come through a rule that takes the first match. Its
    /// order then holds, and of alternatives, the one that goes furthest still
    /// comes first.
    fn ends(&self, expression: &Expression) -> TokenStream {
        let ends = quote! { __caiaphas::Ends };
        if let Some(single) = self.writer.single(expression) {
            return quote! { #ends::of(#single) };
        }
        let keeps_order =
            !self.writer.first_match && self.keeps_order(expression);
        let ways = match &expression.kind {
            ExpressionKind::SingleDefinition(steps) => {
                let steps = steps.iter().map(|step| self.step(step));
                quote! {
                    {
                        let ends = #ends::one(position);
                        #(let ends = #steps;)*
                        ends
                    }
                }
            }
            ExpressionKind::DefinitionsList(alternatives) => {
                let alternatives = alternatives
                    .iter()
                    .map(|alternative| self.branch(alternative));
                let add = if keeps_order {
                    quote! { merge }
                } else {
                    quote! { extend }
                };
                quote! {
                    {
                        let mut ends = #ends::none();
                        #(ends.#add(#alternatives);)*
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
                let mut settled = true;
                syntactic_exception.visit_meta_identifiers(
                    &mut |RuleId(read), _| {
                        settled &= self.settled[read];
                    },
                );
                // An exception whose ends do not hang on what follows is read
                // once, up to the furthest end of the factor, instead of once
                // for each end.
                if settled {
                    return quote! {
                        {
                            let mut ends = #factor;
                            if let ::core::option::Option::Some(furthest) =
                                ends.iter().max()
                            {
                                let excluded = {
                                    let mut within = parser.within(furthest);
                                    let parser = &mut within;
                                    #exception
                                };
                                ends.retain(|end| {
                                    let excluded = excluded.contains(end);
                                    if excluded {
                                        parser.excluded(&RULE, position, end);
                                    }
                                    !excluded
                                });
                            }
                            ends
                        }
                    };
                }
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
        if self.writer.first_match || keeps_order {
            ways
        } else {
            quote! { #ways.longest_first() }
        }
    }

    /// Returns whether the ends of `expression` keep the order of a rule that
    /// takes the first match, since it reads one.
    fn keeps_order(&self, expression: &Expression) -> bool {
        let mut keeps = false;
        expression.visit_meta_identifiers(&mut |RuleId(read), _| {
            keeps |= self.ordered[read];
        });
        keeps
    }

    /// Returns the code that tells where `step` of a sequence can end after
    /// each of `ends`. A run of characters from a set is read once for all of
    /// them, since the ends of a run all reach its end.
    fn step(&self, step: &Expression) -> TokenStream {
        let set = match &step.kind {
            ExpressionKind::RepeatedSequence(sequence) => {
                self.writer.set_parts(sequence)
            }
            _ => None,
        };
        if let Some((pattern, expected)) = set {
            return quote! {
                parser.then_span(
                    &ends,
                    |c| ::core::matches!(c, #pattern),
                    &[#(#expected),*],
                )
            };
        }
        let part = self.part(step);
        quote! { parser.then(&ends, #part) }
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
