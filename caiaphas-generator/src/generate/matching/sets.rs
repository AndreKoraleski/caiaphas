//! Sets are the code of what only ever matches one character and builds no
//! node. Each checks that character against all it can be at once.

use proc_macro2::TokenStream;
use quote::quote;

use super::FunctionWriter;
use crate::{
    analysis::CharacterSet,
    ir::{Expression, ExpressionKind, RuleId},
};

impl FunctionWriter<'_> {
    /// Returns the code of each of `alternatives` as a branch, with each run
    /// of alternatives that only ever match one character and build no node
    /// made into one set. Those alternatives all tie, and only their order
    /// among themselves tells them apart, so taking them as one set matches
    /// the same.
    pub(in crate::generate) fn branches(
        &self,
        alternatives: &[Expression],
    ) -> Vec<TokenStream> {
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

    /// Returns the code of `factor` without `exception` as one set, when both
    /// only ever match one character and the factor builds no node.
    pub(in crate::generate) fn set_except(
        &self,
        factor: &Expression,
        exception: &Expression,
    ) -> Option<TokenStream> {
        let (characters, expected) = self.characters(factor)?;
        let excludes = contains(&self.facts.single_characters(exception)?);
        let includes = contains(&characters);
        Some(quote! {
            parser.set_except(
                position,
                #includes,
                #excludes,
                &[#(#expected),*],
            )
        })
    }

    /// Returns the code of `expression` when it only ever matches one
    /// character and builds no node, as one set unless it is a terminal string
    /// or special sequence, which already check one character.
    pub(in crate::generate) fn single(
        &self,
        expression: &Expression,
    ) -> Option<TokenStream> {
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
    pub(in crate::generate) fn set_parts(
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
    pub(in crate::generate) fn characters(
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
    if let Some(pattern) = pattern(characters) {
        quote! { |c| ::core::matches!(c, #pattern) }
    } else {
        quote! { |_| false }
    }
}

/// Returns the pattern that matches every character of `characters`, if any.
pub(in crate::generate) fn pattern(
    characters: &CharacterSet,
) -> Option<TokenStream> {
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
