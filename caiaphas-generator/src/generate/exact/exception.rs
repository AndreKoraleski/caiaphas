//! An exception takes away the ends of its factor that it matches too.

use proc_macro2::TokenStream;
use quote::quote;

use super::ExactWriter;
use crate::ir::{Expression, RuleId};

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
        // The ends of the factor come in its order, and an exception only
        // takes some away.
        let factor = self.ends(syntactic_factor);
        let exception = self.ends(syntactic_exception);
        let mut settled = true;
        syntactic_exception.visit_meta_identifiers(&mut |RuleId(read), _| {
            settled &= self.settled[read];
        });
        // An exception whose ends do not hang on what follows is read once, up
        // to the furthest end of the factor, instead of once for each end.
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
}
