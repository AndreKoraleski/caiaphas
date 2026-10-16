//! Exact matching writes the two functions of each rule that tries every way
//! through it. One tells where the rule's matches at a position can end, in
//! the order the rule prefers them. The other builds the node of the match
//! from a position to one of those ends.
//!
//! Code that tells where an expression can end reads `parser` and `position`,
//! as in matching, and gives its ends. Code that builds an expression's match
//! also reads `end`, and adds the nodes of the rules matched inside it. Each
//! part of the match takes the first end it prefers that the rest of the match
//! can go on from to `end`.

use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

use crate::{
    generate::{
        matching::{self, FunctionWriter, pattern},
        name,
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
    /// Writes the code of what only ever matches one character.
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
        let RuleId(position) = rule_id;
        let number = Literal::usize_unsuffixed(position);
        let name = name::spaced(rule);
        let hidden = rule.hidden;
        let constant = quote! {
            const RULE: __caiaphas::Rule = __caiaphas::Rule {
                number: #number,
                name: #name,
                hidden: #hidden,
                remembers: true,
            };
        };
        let body = self.ends(&rule.expression);
        let ends_function = quote! {
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
                parser.finish_ends(&RULE, start, ends)
            }
        };
        if hidden {
            return ends_function;
        }
        let building = self.build(&rule.expression);
        quote! {
            #ends_function

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
    pub(super) fn committed(
        &self,
        rule_id: RuleId,
        rule: &Rule,
    ) -> TokenStream {
        let function = matching::function_name(rule_id);
        let ends = ends_name(rule_id);
        let node = if rule.hidden {
            quote! { ::core::option::Option::None }
        } else {
            let build = build_name(rule_id);
            quote! { #build(parser, position, end) }
        };
        quote! {
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
                        #(let ends = parser.then(ends, #steps);)*
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
                            let mut within = parser.within(end);
                            let parser = &mut within;
                            !(#exception).contains(end)
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
        let facts = self.writer.facts;
        let checks_itself = matches!(
            expression.kind,
            ExpressionKind::TerminalString(_)
                | ExpressionKind::SpecialSequence(_)
        ) || self.writer.single(expression).is_some();
        if checks_itself || facts.is_nullable(expression) {
            return code;
        }
        let none = quote! { __caiaphas::Ends::none() };
        let Some(pattern) = pattern(&facts.first(expression)) else {
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

    /// Returns the code that adds the nodes of the rules matched inside
    /// `expression`, in its match from `position` to `end`. It is empty when
    /// the expression builds no node.
    fn build(&self, expression: &Expression) -> TokenStream {
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
