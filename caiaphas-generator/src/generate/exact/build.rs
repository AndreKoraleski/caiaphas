//! Building writes the code that adds the nodes of a match from `position` to
//! `end`, where each part takes the first end it prefers that the rest of the
//! match can go on from.

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

use super::{ExactWriter, build_name};
use crate::{
    generate::matching,
    ir::{Expression, ExpressionKind, RuleId},
};

impl ExactWriter<'_> {
    /// Returns the code that adds the nodes of the rules matched inside
    /// `expression`, in its match from `position` to `end`. It is empty when
    /// the expression builds no node.
    pub(super) fn build(&self, expression: &Expression) -> TokenStream {
        if !self.builds_nodes(expression) {
            return TokenStream::new();
        }
        match &expression.kind {
            ExpressionKind::SingleDefinition(steps) => self.build_steps(steps),
            ExpressionKind::DefinitionsList(alternatives) => {
                // Alternatives that end alike tie, and the first written of
                // them wins.
                let (last, earlier) = alternatives
                    .split_last()
                    .expect("a definitions list has alternatives");
                let tests = earlier.iter().map(|earlier| self.branch(earlier));
                let builds = earlier.iter().map(|earlier| self.build(earlier));
                let last = self.build(last);
                quote! {
                    #(if (#tests).contains(end) { #builds } else)* { #last }
                }
            }
            ExpressionKind::OptionalSequence(sequence) => {
                let build = self.build(sequence);
                if self.writer.facts.is_nullable(sequence) {
                    let ends = self.ends(sequence);
                    quote! { if (#ends).contains(end) { #build } }
                } else {
                    quote! { if end != position { #build } }
                }
            }
            ExpressionKind::RepeatedSequence(sequence) => {
                let round = self.branch(sequence);
                let build = self.build(sequence);
                quote! {
                    {
                        let round = parser.part(|parser, position| #round);
                        let mut position = position;
                        for next in parser.rounds_to(position, end, round) {
                            {
                                let end = next;
                                #build
                            }
                            position = next;
                        }
                    }
                }
            }
            ExpressionKind::SyntacticFactor {
                integer,
                syntactic_primary,
            } => {
                let integer = Literal::u32_unsuffixed(*integer);
                let step = self.part(syntactic_primary);
                let build = self.build(syntactic_primary);
                quote! {
                    {
                        let step = parser.part(#step);
                        let mut position = position;
                        for left in (0..#integer).rev() {
                            let candidates = step(parser, position);
                            let next = parser.choose(
                                candidates,
                                |parser, next| {
                                    let ends = __caiaphas::Ends::one(next);
                                    let ends = parser.times(ends, left, step);
                                    ends.contains(end)
                                },
                            );
                            {
                                let end = next;
                                #build
                            }
                            position = next;
                        }
                    }
                }
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor, ..
            } => self.build(syntactic_factor),
            ExpressionKind::MetaIdentifier(rule_id) => {
                let RuleId(position) = *rule_id;
                if self.general[position] {
                    let function = build_name(*rule_id);
                    return quote! {
                        {
                            let node = #function(parser, position, end);
                            parser.add_node(node);
                        }
                    };
                }
                // The rule remembers its match from telling where it ends.
                let function = matching::function_name(*rule_id);
                quote! {
                    {
                        let found = #function(parser, position);
                        parser.matched(found);
                    }
                }
            }
            ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::EmptySequence => TokenStream::new(),
        }
    }

    /// Returns the code that adds the nodes of `steps`, matched one after
    /// another from `position` to `end`. Each step takes the first end it
    /// prefers that the steps after it can go on from to `end`, up to the last
    /// step that builds a node.
    fn build_steps(&self, steps: &[Expression]) -> TokenStream {
        let Some(last) =
            steps.iter().rposition(|step| self.builds_nodes(step))
        else {
            return TokenStream::new();
        };
        let parts: Vec<Ident> = (0..steps.len())
            .map(|index| format_ident!("part_{index}"))
            .collect();
        // Which steps have their closure read, as where they can end or as
        // part of what comes after a step that chooses.
        let mut read = vec![false; steps.len()];
        let mut code = Vec::new();
        for (index, step) in steps.iter().enumerate().take(last + 1) {
            let build = self.build(step);
            if index + 1 == steps.len() {
                code.push(build);
                break;
            }
            read[index] = true;
            let part = &parts[index];
            let middle = if self.has_one_end(step) {
                quote! { #part(parser, position).only() }
            } else {
                let rest = &parts[index + 1..];
                for later in &mut read[index + 1..] {
                    *later = true;
                }
                quote! {
                    {
                        let candidates = #part(parser, position);
                        parser.choose(candidates, |parser, middle| {
                            let ends = __caiaphas::Ends::one(middle);
                            #(let ends = parser.then(ends, #rest);)*
                            ends.contains(end)
                        })
                    }
                }
            };
            // Only the steps up to the last that builds a node read where the
            // step before them ends.
            let next =
                (index < last).then(|| quote! { let position = middle; });
            let build = (!build.is_empty()).then(|| {
                quote! {
                    {
                        let end = middle;
                        #build
                    }
                }
            });
            code.push(quote! {
                let middle = #middle;
                #build
                #next
            });
        }
        let closures = steps
            .iter()
            .zip(&parts)
            .zip(&read)
            .filter(|(_, read)| **read)
            .map(|((step, part), _)| {
                let closure = self.part(step);
                quote! { let #part = parser.part(#closure); }
            });
        quote! {
            {
                #(#closures)*
                #(#code)*
            }
        }
    }

    /// Returns whether `expression` builds the node of any rule. A hidden rule
    /// has none, and neither does an exception.
    fn builds_nodes(&self, expression: &Expression) -> bool {
        match &expression.kind {
            ExpressionKind::MetaIdentifier(RuleId(position)) => {
                !self.writer.facts.grammar().syntax_rules[*position].hidden
            }
            ExpressionKind::SyntacticTerm {
                syntactic_factor, ..
            } => self.builds_nodes(syntactic_factor),
            _ => expression.children().any(|child| self.builds_nodes(child)),
        }
    }

    /// Returns whether `expression` has at most one end at any position. A
    /// step that has one goes on from it without choosing.
    fn has_one_end(&self, expression: &Expression) -> bool {
        if self.writer.single(expression).is_some() {
            return true;
        }
        match &expression.kind {
            ExpressionKind::SingleDefinition(steps) => {
                steps.iter().all(|step| self.has_one_end(step))
            }
            ExpressionKind::SyntacticFactor {
                syntactic_primary: expression,
                ..
            }
            | ExpressionKind::SyntacticTerm {
                syntactic_factor: expression,
                ..
            } => self.has_one_end(expression),
            ExpressionKind::MetaIdentifier(RuleId(position)) => {
                !self.general[*position]
            }
            ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::EmptySequence => true,
            ExpressionKind::DefinitionsList(_)
            | ExpressionKind::OptionalSequence(_)
            | ExpressionKind::RepeatedSequence(_) => false,
        }
    }
}
