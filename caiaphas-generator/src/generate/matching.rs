//! Matching writes the function that matches each rule against an input with
//! the parser of the runtime.
//!
//! Each function reads `parser` and `position`, and so does the code of each
//! expression in it. That code gives where the expression's match ends, if it
//! matches. An expression that goes on from where another ends binds
//! `position` again, so the code of every expression fits anywhere.

mod expression;
mod sets;

pub use sets::Expectation;
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
/// reads it. Only the rules that `builds` marks build a node.
pub(crate) fn functions(
    facts: &Facts<'_>,
    general: &[bool],
    builds: &[bool],
) -> TokenStream {
    let grammar: &Grammar = facts.grammar();
    let Decisions {
        remembers,
        committed,
        leaves,
        recursive,
        settled,
    } = Decisions::new(facts, general, builds);
    let functions =
        grammar
            .syntax_rules
            .iter()
            .enumerate()
            .map(|(position, rule)| {
                let rule_id = RuleId(position);
                let writer = FunctionWriter { facts, builds };
                if !general[position] {
                    return writer.function(
                        rule_id,
                        rule,
                        remembers[position],
                        leaves[position],
                        recursive[position],
                    );
                }
                let writer = ExactWriter {
                    writer,
                    general,
                    settled: &settled,
                    recursive: &recursive,
                };
                let committing = committed[position]
                    .then(|| ExactWriter::committed(rule_id, rule));
                let functions = writer.functions(rule_id, rule);
                quote! { #functions #committing }
            });
    quote! { #(#functions)* }
}

/// What the code of each rule of a grammar depends on besides the rule itself,
/// by the rule's position.
pub(in crate::generate) struct Decisions {
    /// Whether the rule remembers its match at each position, since it can be
    /// tried there again.
    pub(in crate::generate) remembers: Vec<bool>,
    /// Whether the rule tries every way and a rule that takes each way as it
    /// comes reads it.
    pub(in crate::generate) committed: Vec<bool>,
    /// Whether the rule builds no node and reads no rule that does.
    pub(in crate::generate) leaves: Vec<bool>,
    /// Whether the rule can reach itself.
    pub(in crate::generate) recursive: Vec<bool>,
    /// What [`exact::settled_rules`] tells.
    pub(in crate::generate) settled: Vec<bool>,
}

impl Decisions {
    /// Returns the decisions about the grammar `facts` answers about, where
    /// `general` marks the rules that try every way and `builds` those that
    /// build a node.
    pub(in crate::generate) fn new(
        facts: &Facts<'_>,
        general: &[bool],
        builds: &[bool],
    ) -> Self {
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
        // A rule that builds no node and reads none that does needs no account
        // of the nodes matched inside it.
        let leaves = (0..grammar.syntax_rules.len())
            .map(|position| {
                let reached = grammar.reached_from([RuleId(position)]);
                reached
                    .iter()
                    .zip(builds)
                    .all(|(&reached, &builds)| !(reached && builds))
            })
            .collect();
        Self {
            remembers,
            committed,
            leaves,
            recursive: grammar.recursive_rules(),
            settled: exact::settled_rules(facts, general),
        }
    }
}

/// Returns `body`, the code of a rule's function that gives an `output`, kept
/// from nesting too deeply when the rule is `recursive`: past the depth a
/// parser follows, the function gives `failure` instead.
pub(super) fn nested(
    recursive: bool,
    output: &TokenStream,
    failure: &TokenStream,
    body: &TokenStream,
) -> TokenStream {
    if !recursive {
        return body.clone();
    }
    quote! {
        if !parser.enter(position) {
            return #failure;
        }
        let found = (|| -> #output { #body })();
        parser.leave();
        found
    }
}

/// Returns whether what `rule` could not read never tells why an input does
/// not match: it is hidden and can match nothing, so a parser can always go on
/// without it. One that looks ahead can still fail where it reads nothing.
pub(super) fn quiet(facts: &Facts<'_>, rule: &Rule) -> bool {
    rule.hidden
        && facts.is_nullable(&rule.expression)
        && !facts.looks_ahead(&rule.expression)
}

/// Returns the name of the function of the rule `rule_id`.
pub(crate) fn function_name(RuleId(position): RuleId) -> Ident {
    format_ident!("rule_{position}")
}

/// Writes the function of one rule.
#[derive(Clone, Copy)]
pub(super) struct FunctionWriter<'a> {
    pub(super) facts: &'a Facts<'a>,
    /// Which rules build a node, since a type reads it.
    pub(super) builds: &'a [bool],
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
        let builds = self.builds[position];
        let quiet = quiet(self.facts, rule);
        let start = self.facts.grammar().start_syntax_rules.contains(&rule_id);
        quote! {
            const RULE: __caiaphas::Rule = __caiaphas::Rule {
                number: #number,
                name: #name,
                hidden: #hidden,
                builds: #builds,
                quiet: #quiet,
                inline: #inline,
                remembers: #remembers,
                start: #start,
            };
        }
    }

    /// Returns the function of `rule`. It remembers the rule's matches when
    /// `remembers` holds. A `leaf` builds no node and reads no rule that does,
    /// and a `recursive` rule can reach itself.
    fn function(
        &self,
        rule_id: RuleId,
        rule: &Rule,
        remembers: bool,
        leaf: bool,
        recursive: bool,
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
        // A rule that recovers goes on from where a parse that recovers tells
        // it to, when it fails.
        let mut finish = if rule.recovers {
            quote! {
                let found = parser.finish_rule(&RULE, start, end);
                if found.is_some() {
                    return found;
                }
                parser.recover(&RULE, start, #function::<REPORTS, W>)
            }
        } else {
            quote! { parser.finish_rule(&RULE, start, end) }
        };
        let mut start = quote! { let start = parser.start_rule(position); };
        // A parse that only matches needs no account of where a leaf starts,
        // since it has no nodes to keep or drop.
        if leaf && !remembers && !rule.recovers {
            start = quote! {
                let start = REPORTS.then(|| parser.start_rule(position));
            };
            finish = quote! {
                match start {
                    ::core::option::Option::Some(start) => #finish,
                    ::core::option::Option::None => {
                        end.map(|end| (end, ::core::option::Option::None))
                    }
                }
            };
        }
        let matching = nested(
            recursive,
            &quote! { __caiaphas::Match },
            &quote! { ::core::option::Option::None },
            &quote! {
                #recall
                #start
                let end = #body;
                #convert
                #finish
            },
        );
        quote! {
            #[doc(hidden)]
            fn #function<const REPORTS: bool, W: __caiaphas::Width>(
                parser: &mut __caiaphas::Parser<'_, REPORTS, W>,
                position: usize,
            ) -> __caiaphas::Match {
                #constant
                #matching
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
