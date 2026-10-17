//! An exception takes away the ends of its factor that it matches too.

use proc_macro2::{Literal, TokenStream};
use quote::quote;

use super::ExactWriter;
use crate::ir::{Expression, ExpressionKind, RuleId};

impl ExactWriter<'_> {
    /// Returns the code that tells where `syntactic_factor` can end, less the
    /// ends `syntactic_exception` matches up to as well.
    pub(super) fn except(
        &self,
        syntactic_factor: &Expression,
        syntactic_exception: &Expression,
    ) -> TokenStream {
        let ends = quote! { __caiaphas::Ends };
        if let Some(code) = self
            .writer
            .set_except(syntactic_factor, syntactic_exception)
        {
            return quote! { #ends::of(#code) };
        }
        if let Some(code) =
            self.before_middle(syntactic_factor, syntactic_exception)
        {
            return code;
        }
        // The ends of the factor come in its order, and an exception only
        // takes some away.
        let factor = self.ends(syntactic_factor);
        let exception = self.ends(syntactic_exception);
        // An exception whose ends do not hang on what follows is read once, up
        // to the furthest end of the factor, instead of once for each end.
        if self.settled_exception(syntactic_exception) {
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
        quote! {
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
        }
    }

    /// Returns the code of an exception that takes away any run of the
    /// characters of its factor holding a match of the exception's middle, as
    /// [`run_and_middle`](Self::run_and_middle) tells. One scan reads the run
    /// up to the first such match, where reading the factor and the exception
    /// would each read all of it.
    fn before_middle(
        &self,
        factor: &Expression,
        exception: &Expression,
    ) -> Option<TokenStream> {
        let (run, middle) = self.run_and_middle(factor, exception)?;
        let (pattern, expected) = self.writer.set_parts(run)?;
        // The place of the factor in the grammar tells this run from others.
        let site = Literal::usize_unsuffixed(factor.span.range().start);
        let steps = middle.iter().map(|step| self.step(step));
        Some(quote! {
            parser.span_ends_before(
                &RULE,
                #site,
                position,
                |c| ::core::matches!(c, #pattern),
                &[#(#expected),*],
                |parser, position| {
                    let ends = __caiaphas::Ends::one(position);
                    #(let ends = #steps;)*
                    ends
                },
            )
        })
    }

    /// Returns what `factor` repeats and the middle steps of `exception`, when
    /// `factor` is a run of characters such as `{c}` and `exception` takes
    /// away any run holding a match of its middle, as `{c}, middle, {c}` does.
    /// The repetitions around the middle take every character of the run, so
    /// the exception matches a run exactly when a match of the middle lies
    /// inside it. The exception must not hang on what follows, and its middle
    /// must only ever match a little text, so trying it at each place costs
    /// little.
    pub(in crate::generate) fn run_and_middle<'e>(
        &self,
        factor: &'e Expression,
        exception: &'e Expression,
    ) -> Option<(&'e Expression, &'e [Expression])> {
        let facts = self.writer.facts;
        let ExpressionKind::RepeatedSequence(run) = &factor.kind else {
            return None;
        };
        let characters = facts.single_characters(run)?;
        self.writer.set_parts(run)?;
        let ExpressionKind::SingleDefinition(steps) = &exception.kind else {
            return None;
        };
        let [first, middle @ .., last] = steps.as_slice() else {
            return None;
        };
        let takes_the_run = |step: &Expression| match &step.kind {
            ExpressionKind::RepeatedSequence(repeated) => facts
                .single_characters(repeated)
                .is_some_and(|taken| characters.is_subset(&taken)),
            _ => false,
        };
        let found = !middle.is_empty()
            && takes_the_run(first)
            && takes_the_run(last)
            && middle.iter().all(|step| self.short(step))
            && self.settled_exception(exception);
        found.then_some((run.as_ref(), middle))
    }

    /// Returns whether the ends of `exception` do not hang on what follows.
    fn settled_exception(&self, exception: &Expression) -> bool {
        let mut settled = true;
        exception.visit_meta_identifiers(&mut |RuleId(read), _| {
            settled &= self.settled[read];
        });
        settled
    }

    /// Returns whether every match of `expression` is short, since it repeats
    /// nothing. An exception reads no rule that can reach itself, so looking
    /// into the rules it reads always ends.
    fn short(&self, expression: &Expression) -> bool {
        match &expression.kind {
            ExpressionKind::RepeatedSequence(_) => false,
            ExpressionKind::MetaIdentifier(RuleId(read)) => {
                let rules = &self.writer.facts.grammar().syntax_rules;
                self.short(&rules[*read].expression)
            }
            ExpressionKind::SingleDefinition(parts)
            | ExpressionKind::DefinitionsList(parts) => {
                parts.iter().all(|part| self.short(part))
            }
            ExpressionKind::OptionalSequence(part)
            | ExpressionKind::SyntacticFactor {
                syntactic_primary: part,
                ..
            }
            | ExpressionKind::SyntacticTerm {
                syntactic_factor: part,
                ..
            } => self.short(part),
            ExpressionKind::TerminalString(_)
            | ExpressionKind::SpecialSequence(_)
            | ExpressionKind::EmptySequence => true,
        }
    }
}
