//! Matching writes the function of each rule, which matches the rule against
//! an input with the parser of the runtime.
//!
//! Each function reads `parser` and `position`, and so does the code of each
//! expression in it, which gives where the expression's match ends, if it
//! matches. An expression that goes on from where another ends binds
//! `position` again, so the code of every expression fits anywhere.

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

use crate::{
    analysis::{CharacterSet, Facts, revisited_rules},
    generate::name,
    ir::{Expression, ExpressionKind, Grammar, Rule, RuleId},
};

/// Returns the function of every rule of the grammar `facts` answers about.
pub(crate) fn functions(facts: &Facts<'_>) -> TokenStream {
    let grammar: &Grammar = facts.grammar();
    let revisited = revisited_rules(facts);
    let functions =
        grammar.syntax_rules.iter().zip(revisited).enumerate().map(
            |(position, (rule, remembers))| {
                FunctionWriter {
                    facts,
                    first_match: rule.first_match,
                }
                .function(RuleId(position), rule, remembers)
            },
        );
    quote! { #(#functions)* }
}

/// Returns the name of the function of the rule `rule_id`.
pub(crate) fn function_name(RuleId(position): RuleId) -> Ident {
    format_ident!("rule_{position}")
}

/// Writes the function of one rule.
struct FunctionWriter<'a> {
    facts: &'a Facts<'a>,
    /// Whether the rule's choices take their first alternative that matches.
    first_match: bool,
}

impl FunctionWriter<'_> {
    /// Returns the function of `rule`, which `remembers` its matches or not.
    fn function(
        &self,
        rule_id: RuleId,
        rule: &Rule,
        remembers: bool,
    ) -> TokenStream {
        let function = function_name(rule_id);
        let RuleId(position) = rule_id;
        let number = Literal::usize_unsuffixed(position);
        let name = name::spaced(rule);
        let hidden = rule.hidden;
        let body = self.expression(&rule.expression);
        let recall = remembers.then(|| {
            quote! {
                if let ::core::option::Option::Some(found) =
                    parser.recall(&RULE, position)
                {
                    return found;
                }
            }
        });
        quote! {
            fn #function<const REPORTS: bool>(
                parser: &mut __caiaphas::Parser<'_, REPORTS>,
                position: usize,
            ) -> __caiaphas::Match {
                const RULE: __caiaphas::Rule = __caiaphas::Rule {
                    number: #number,
                    name: #name,
                    hidden: #hidden,
                    remembers: #remembers,
                };
                #recall
                let start = parser.start_rule(position);
                let end = #body;
                parser.finish_rule(&RULE, start, end)
            }
        }
    }

    /// Returns the code of `expression`.
    fn expression(&self, expression: &Expression) -> TokenStream {
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
            ExpressionKind::DefinitionsList(expressions)
                if self.first_match =>
            {
                let branches =
                    expressions.iter().map(|branch| self.branch(branch));
                quote! {
                    (|| {
                        let mark = parser.mark();
                        #(
                            if let #some(end) = #branches {
                                return #some(end);
                            }
                            parser.truncate(mark);
                        )*
                        #none
                    })()
                }
            }
            ExpressionKind::DefinitionsList(expressions) => {
                let branches =
                    expressions.iter().map(|branch| self.branch(branch));
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
            // The analysis makes sure the body of a repeated sequence cannot
            // match nothing, so each round goes on.
            ExpressionKind::RepeatedSequence(expression) => {
                let branch = self.branch(expression);
                quote! {
                    {
                        let mut end = position;
                        loop {
                            let position = end;
                            let mark = parser.mark();
                            match #branch {
                                #some(next) => end = next,
                                #none => {
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
                        if excluded { #none } else { #some(end) }
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
            ExpressionKind::EmptySequence => quote! { #some(position) },
        }
    }

    /// Returns the code of `expression` as a branch a parser can take or not,
    /// which a parse that only matches skips when the next character is none
    /// of those the branch can start with.
    ///
    /// A branch that can match nothing can start anywhere, and a terminal
    /// string or special sequence already fails on its first character, so
    /// those are left as they are.
    fn branch(&self, expression: &Expression) -> TokenStream {
        let code = self.expression(expression);
        let checks_itself = matches!(
            expression.kind,
            ExpressionKind::TerminalString(_)
                | ExpressionKind::SpecialSequence(_)
        );
        if checks_itself || self.facts.is_nullable(expression) {
            return code;
        }
        let Some(pattern) = pattern(&self.facts.first(expression)) else {
            // A branch that cannot start with any character never matches.
            return quote! {
                if REPORTS { #code } else { ::core::option::Option::None }
            };
        };
        quote! {
            if REPORTS
                || ::core::matches!(
                    parser.character(position),
                    ::core::option::Option::Some(#pattern)
                )
            {
                #code
            } else {
                ::core::option::Option::None
            }
        }
    }
}

/// Returns the pattern that matches every character of `characters`, if any.
fn pattern(characters: &CharacterSet) -> Option<TokenStream> {
    let ranges = characters.ranges().iter().map(|range| {
        let (first, last) = (range.start(), range.end());
        if first == last {
            quote! { #first }
        } else {
            quote! { #first..=#last }
        }
    });
    (!characters.is_empty()).then(|| quote! { #(#ranges)|* })
}
