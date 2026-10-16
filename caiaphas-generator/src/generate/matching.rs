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
                let excluded =
                    self.facts.single_characters(syntactic_exception);
                if let (Some((characters, expected)), Some(excluded)) =
                    (self.characters(syntactic_factor), excluded)
                {
                    let excludes = contains(&excluded);
                    let includes = contains(&characters);
                    return quote! {
                        parser.set_except(
                            position,
                            #includes,
                            #excludes,
                            &[#(#expected),*],
                        )
                    };
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

    /// Returns the code of each of `alternatives` as a branch, with each run
    /// of alternatives that only ever match one character and build no node
    /// made into one set. Those alternatives all tie, and only their order
    /// among themselves tells them apart, so taking them as one set matches
    /// the same.
    fn branches(&self, alternatives: &[Expression]) -> Vec<TokenStream> {
        let mut branches = Vec::new();
        let mut run: Vec<&Expression> = Vec::new();
        for alternative in alternatives {
            if self.set_parts(alternative).is_some() {
                run.push(alternative);
                continue;
            }
            if !run.is_empty() {
                branches.push(self.run(&run));
                run.clear();
            }
            branches.push(self.branch(alternative));
        }
        if !run.is_empty() {
            branches.push(self.run(&run));
        }
        branches
    }

    /// Returns the code of the alternatives of `run` as one set, or of its
    /// only alternative as it is.
    fn run(&self, run: &[&Expression]) -> TokenStream {
        if let [alternative] = run {
            return self.branch(alternative);
        }
        let mut characters = CharacterSet::default();
        let mut expected = Vec::new();
        for alternative in run {
            let (set, parts) = self
                .characters(alternative)
                .expect("an alternative of a run is a set");
            characters = characters.union(&set);
            expected.extend(parts);
        }
        set_code(&characters, &expected)
    }

    /// Returns the code of `expression` when it only ever matches one
    /// character and builds no node, as one set unless it is a terminal string
    /// or special sequence, which already check one character.
    fn single(&self, expression: &Expression) -> Option<TokenStream> {
        let (characters, expected) = self.characters(expression)?;
        Some(match expression.kind {
            ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_) => {
                self.expression(expression)
            }
            _ => set_code(&characters, &expected),
        })
    }

    /// Returns the pattern of the characters `expression` matches and what a
    /// parser notes as expected where it fails, when it only ever matches one
    /// character and builds no node.
    fn set_parts(
        &self,
        expression: &Expression,
    ) -> Option<(TokenStream, Vec<TokenStream>)> {
        let (characters, expected) = self.characters(expression)?;
        Some((pattern(&characters)?, expected))
    }

    /// Returns the characters `expression` matches and what a parser notes as
    /// expected where it fails, when it only ever matches one character and
    /// builds no node.
    ///
    /// A hidden rule notes nothing where it fails to start, so it adds nothing
    /// to note. A syntactic term is left out, since the characters its
    /// exception takes out fail without noting anything.
    fn characters(
        &self,
        expression: &Expression,
    ) -> Option<(CharacterSet, Vec<TokenStream>)> {
        let characters = self.facts.single_characters(expression)?;
        let expected = match &expression.kind {
            ExpressionKind::TerminalString(text) => {
                vec![quote! { __caiaphas::Expected::Text(#text) }]
            }
            ExpressionKind::SpecialSequence(range) => {
                let (first, last) = (range.start(), range.end());
                vec![quote! { __caiaphas::Expected::Range(#first, #last) }]
            }
            ExpressionKind::DefinitionsList(alternatives) => {
                let mut expected = Vec::new();
                for alternative in alternatives {
                    expected.extend(self.characters(alternative)?.1);
                }
                expected
            }
            ExpressionKind::MetaIdentifier(RuleId(position)) => {
                let rule = &self.facts.grammar().syntax_rules[*position];
                if !rule.hidden {
                    return None;
                }
                self.characters(&rule.expression)?;
                Vec::new()
            }
            _ => return None,
        };
        Some((characters, expected))
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

/// Returns the code that matches one of `characters`, noting each of
/// `expected` where it fails.
fn set_code(
    characters: &CharacterSet,
    expected: &[TokenStream],
) -> TokenStream {
    let contains = contains(characters);
    quote! { parser.set(position, #contains, &[#(#expected),*]) }
}

/// Returns the closure that tells whether a character is one of `characters`.
fn contains(characters: &CharacterSet) -> TokenStream {
    match pattern(characters) {
        Some(pattern) => quote! { |c| ::core::matches!(c, #pattern) },
        None => quote! { |_| false },
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
