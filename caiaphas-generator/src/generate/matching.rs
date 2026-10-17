//! Matching writes the function that matches each rule against an input with
//! the parser of the runtime.
//!
//! Each function reads `parser` and `position`, and so does the code of each
//! expression in it. That code gives where the expression's match ends, if it
//! matches. An expression that goes on from where another ends binds
//! `position` again, so the code of every expression fits anywhere.

mod expression;
mod sets;

pub(super) use sets::pattern;

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

use crate::{
    analysis::{Facts, revisited_rules},
    generate::{
        code,
        exact::{self, ExactWriter},
        name,
    },
    ir::{Expression, ExpressionKind, Grammar, Rule, RuleId},
};

/// Returns the functions of every rule of the grammar `facts` answers about,
/// where those that `general` marks try every way through them.
///
/// Every other rule that one of those reads remembers its matches, since it is
/// read once to tell where it ends and again to build its node. A rule that
/// tries every way also gets a function that takes one way when a greedy rule
/// reads it.
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
    let ordered = exact::ordered_rules(grammar, general);
    let settled = exact::settled_rules(facts, general);
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
                let writer = ExactWriter {
                    writer,
                    general,
                    ordered: &ordered,
                    settled: &settled,
                };
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
    /// Returns the constant that tells the parser about `rule`, numbered
    /// `rule_id`, when its function `remembers` its matches or not.
    pub(super) fn rule_constant(
        &self,
        rule_id: RuleId,
        rule: &Rule,
        remembers: bool,
    ) -> TokenStream {
        let RuleId(position) = rule_id;
        let number = Literal::usize_unsuffixed(position);
        let name = name::spaced(rule);
        let (hidden, inline) = (rule.hidden, rule.inline);
        let quiet = hidden && self.facts.is_nullable(&rule.expression);
        let start = self.facts.grammar().start_syntax_rules.contains(&rule_id);
        quote! {
            const RULE: __caiaphas::Rule = __caiaphas::Rule {
                number: #number,
                name: #name,
                hidden: #hidden,
                quiet: #quiet,
                inline: #inline,
                remembers: #remembers,
                start: #start,
            };
        }
    }

    /// Returns the function of `rule`. It remembers the rule's matches when
    /// `remembers` holds.
    fn function(
        &self,
        rule_id: RuleId,
        rule: &Rule,
        remembers: bool,
    ) -> TokenStream {
        let function = function_name(rule_id);
        let constant = self.rule_constant(rule_id, rule, remembers);
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
            #[doc(hidden)]
            fn #function<const REPORTS: bool>(
                parser: &mut __caiaphas::Parser<'_, REPORTS>,
                position: usize,
            ) -> __caiaphas::Match {
                #constant
                #recall
                let start = parser.start_rule(position);
                let end = #body;
                #convert
                parser.finish_rule(&RULE, start, end)
            }
        }
    }

    /// Returns the code of `expression` as a branch a parser can take or not.
    /// A parse that only matches skips it when the next character is none of
    /// those the branch can start with.
    ///
    /// A branch that can match nothing can start anywhere, and a terminal
    /// string or special sequence already fails on its first character, so
    /// those are left as they are.
    fn branch(&self, expression: &Expression) -> TokenStream {
        let code = self.expression(expression);
        let none = quote! { ::core::option::Option::None };
        self.guarded(expression, code, &none, false)
    }

    /// Returns `code`, the code of the branch `expression`, behind a check of
    /// the next character, so a parse that only matches gives `none` where the
    /// branch cannot start. A branch that `checks_itself` on its first
    /// character is left as it is.
    pub(super) fn guarded(
        &self,
        expression: &Expression,
        code: TokenStream,
        none: &TokenStream,
        checks_itself: bool,
    ) -> TokenStream {
        let checks_itself = checks_itself
            || matches!(
                expression.kind,
                ExpressionKind::TerminalString(_)
                    | ExpressionKind::SpecialSequence(_)
            );
        if checks_itself || self.facts.is_nullable(expression) {
            return code;
        }
        let Some(pattern) = pattern(&self.facts.first(expression)) else {
            // A branch that cannot start with any character never matches.
            return quote! { if REPORTS { #code } else { #none } };
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
                #none
            }
        }
    }
}
