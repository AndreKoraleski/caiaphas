//! The `grammar!` macro of caiaphas, which the `caiaphas` crate re-exports.

use std::{env, fs, path::Path};

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Ident, LitStr, Token, Visibility,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// What `grammar!` is called with: the module to generate and the file of the
/// grammar it parses.
struct Input {
    attributes: Vec<Attribute>,
    visibility: Visibility,
    name: Ident,
    path: LitStr,
    /// Whether the suggestions about the grammar stop the build, which
    /// `suggest` after the path asks for.
    suggest: bool,
    /// Whether the parser matches the grammar rearranged, which `optimize`
    /// after the path asks for.
    optimize: bool,
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attributes = input.call(Attribute::parse_outer)?;
        let visibility = input.parse()?;
        input.parse::<Token![mod]>()?;
        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let path = input.parse()?;
        let (mut suggest, mut optimize) = (false, false);
        while input.parse::<Option<Token![,]>>()?.is_some() {
            let flag: Ident = input.parse()?;
            match flag.to_string().as_str() {
                "suggest" => suggest = true,
                "optimize" => optimize = true,
                _ => {
                    let message = "expected `suggest` or `optimize`";
                    return Err(syn::Error::new(flag.span(), message));
                }
            }
        }
        input.parse::<Option<Token![;]>>()?;
        Ok(Self {
            attributes,
            visibility,
            name,
            path,
            suggest,
            optimize,
        })
    }
}

/// Generates a module that parses the grammar in a file, with a type for each
/// of its rules.
///
/// It takes the module's attributes and visibility, its name and the path of
/// the grammar from the root of the crate:
///
/// ```text
/// caiaphas::grammar!(pub mod json = "grammars/json.ebnf");
/// ```
///
/// With `suggest` after the path, every change that would make the parser
/// faster stops the build, as a problem in the grammar does:
///
/// ```text
/// caiaphas::grammar!(pub mod json = "grammars/json.ebnf", suggest);
/// ```
///
/// With `optimize`, the parser matches the grammar rearranged wherever that
/// keeps its types the same, such as with a start that alternatives share
/// written once, and those suggestions are left out:
///
/// ```text
/// caiaphas::grammar!(pub mod json = "grammars/json.ebnf", optimize);
/// ```
///
/// The crate documentation of `caiaphas` tells what the module holds.
#[proc_macro]
pub fn grammar(input: TokenStream) -> TokenStream {
    let Input {
        attributes,
        visibility,
        name,
        path,
        suggest,
        optimize,
    } = parse_macro_input!(input as Input);
    let relative = path.value();
    // A grammar's path is relative to the root of the crate that uses it,
    // unlike the path of `include_str!`, which is relative to the file.
    let root = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let file = Path::new(&root).join(&relative);
    let source = match fs::read_to_string(&file) {
        Ok(source) => source,
        Err(error) => {
            let message = format!("cannot read `{relative}`: {error}");
            return syn::Error::new(path.span(), message)
                .into_compile_error()
                .into();
        }
    };
    let errors = |problems: Vec<String>| -> TokenStream {
        problems
            .into_iter()
            .map(|problem| {
                syn::Error::new(path.span(), problem).into_compile_error()
            })
            .collect::<proc_macro2::TokenStream>()
            .into()
    };
    let options = caiaphas_generator::Options { optimize };
    let module =
        match caiaphas_generator::generate(&source, &relative, options) {
            Ok(module) if suggest && !module.suggestions.is_empty() => {
                return errors(module.suggestions);
            }
            Ok(module) => module,
            Err(problems) => return errors(problems),
        };
    let documentation = (!attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("doc")))
    .then(|| {
        let documentation =
            format!(" The parser and types of the grammar in `{relative}`.");
        quote! { #[doc = #documentation] }
    });
    let suggestions = (!module.suggestions.is_empty()).then(|| {
        let documentation = format!(
            "\n # Suggestions\n\n These changes to `{relative}` would make \
             its parser faster:\n\n```text\n{}\n```",
            module.suggestions.join("\n")
        );
        quote! { #[doc = #documentation] }
    });
    // Including the grammar's bytes makes Cargo rebuild when it changes.
    let tracked = file.to_string_lossy();
    let items = module.items;
    quote! {
        #documentation
        #(#attributes)*
        #suggestions
        #visibility mod #name {
            const _: &[u8] = ::core::include_bytes!(#tracked);

            #items
        }
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_module_and_the_path() {
        let input: Input =
            syn::parse_str("#[doc = \"x\"] pub mod json = \"json.ebnf\";")
                .unwrap();
        assert_eq!(input.name, "json");
        assert_eq!(input.path.value(), "json.ebnf");
        assert_eq!(input.attributes.len(), 1);
        assert!(!input.suggest);
    }

    #[test]
    fn reads_whether_to_stop_at_suggestions() {
        let input: Input =
            syn::parse_str("mod json = \"json.ebnf\", suggest").unwrap();
        assert!(input.suggest);
        assert!(!input.optimize);
        assert!(syn::parse_str::<Input>("mod json = \"j\", hint").is_err());
    }

    #[test]
    fn reads_every_flag_after_the_path() {
        let input: Input =
            syn::parse_str("mod json = \"j\", optimize, suggest;").unwrap();
        assert!(input.suggest && input.optimize);
    }
}
