//! Matching writes the function that matches each rule against an input with
//! the parser of the runtime.
//!
//! Each function reads `parser` and `position`, and so does the code of each
//! expression in it. That code gives where the expression's match ends, if it
//! matches. An expression that goes on from where another ends binds
//! `position` again, so the code of every expression fits anywhere.

mod sets;

pub(super) use sets::pattern;

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

use crate::{
    analysis::{CharacterSet, Facts, revisited_rules},
    generate::{code, exact::ExactWriter, name},
    ir::{Expression, ExpressionKind, Grammar, Rule, RuleId},
};

/// Returns the functions of every rule of the grammar `facts` answers about,
/// where those that `general` marks try every way through them.
///
/// Every other rule that one of those reads remembers its matches, since it is
/// read once to tell where it ends and again to build its node. One of those
/// that a greedy rule reads also gets a function that takes one way.
pub(crate) fn functions(facts: &Facts<'_>, general: &[bool]) -> TokenStream {
    let grammar: &Grammar = facts.grammar();
    let mut remembers = revisited_rules(facts);
    let mut committed = vec![false; general.len()];
    for (rule, &is_general) in grammar.syntax_rules.iter().zip(general) {
        rule.expression
            .visit_meta_identifiers(&mut |RuleId(read), _| {
                if is_general {
                    remembers[read] = true;
                } else {
                    committed[read] |= general[read];
                }
            });
    }
    let functions =
        grammar
            .syntax_rules
            .iter()
            .enumerate()
            .map(|(position, rule)| {
                let rule_id = RuleId(position);
                let writer = FunctionWriter {
                    facts,
                    first_match: rule.first_match,
                };
                if !general[position] {
                    return writer.function(
                        rule_id,
                        rule,
                        remembers[position],
                    );
                }
                let writer = ExactWriter { writer, general };
                let committing = committed[position]
                    .then(|| ExactWriter::committed(rule_id, rule));
                let functions = writer.functions(rule_id, rule);
                quote! { #functions #committing }
            });
    quote! { #(#functions)* }
}

/// Returns the name of the function of the rule `rule_id`.
pub(crate) fn function_name(RuleId(position): RuleId) -> Ident {
    format_ident!("rule_{position}")
}

/// Writes the function of one rule.
pub(super) struct FunctionWriter<'a> {
    pub(super) facts: &'a Facts<'a>,
    /// Whether the rule's choices take their first alternative that matches.
    pub(super) first_match: bool,
}

impl FunctionWriter<'_> {
    /// Returns the function of `rule`. It remembers the rule's matches when
    /// `remembers` holds.
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
        let (hidden, inline) = (rule.hidden, rule.inline);
        let quiet = hidden && self.facts.is_nullable(&rule.expression);
        let body = self.expression(&rule.expression);
        // A rule whose type its text converts to fails where the type does not
        // read the text.
        let convert = code::value_type(rule).map(|value| {
            quote! {
                let end = end.filter(|&end| {
                    parser.accepts::<#value>(&RULE, position, end)
                });
            }
        });
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
                    quiet: #quiet,
                    inline: #inline,
                    remembers: #remembers,
                };
                #recall
                let start = parser.start_rule(position);
                let end = #body;
                #convert
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
            ExpressionKind::DefinitionsList(expressions) => {
                let branches = self.branches(expressions);
                if let [branch] = branches.as_slice() {
                    return branch.clone();
                }
                if self.first_match {
                    return quote! {
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
                    };
                }
                if self.apart(expressions) {
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
            // The analysis makes sure the body of a repeated sequence cannot
            // match nothing, so each round goes on.
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

    /// Returns whether at most one of `alternatives` can match at any place,
    /// since none can match nothing and no two can start with the same
    /// character. The one that matches is then the longest.
    fn apart(&self, alternatives: &[Expression]) -> bool {
        let mut seen = CharacterSet::default();
        alternatives.iter().all(|alternative| {
            let first = self.facts.first(alternative);
            let apart = !self.facts.is_nullable(alternative)
                && seen.intersection(&first).is_empty();
            seen = seen.union(&first);
            apart
        })
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
