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
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attributes = input.call(Attribute::parse_outer)?;
        let visibility = input.parse()?;
        input.parse::<Token![mod]>()?;
        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let path = input.parse()?;
        input.parse::<Option<Token![;]>>()?;
        Ok(Self {
            attributes,
            visibility,
            name,
            path,
        })
    }
}

/// Generates a module that parses the grammar in a file, with a type for each
/// of its rules. The `caiaphas` crate documents how to use it.
#[proc_macro]
pub fn grammar(input: TokenStream) -> TokenStream {
    let Input {
        attributes,
        visibility,
        name,
        path,
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
    let items = match caiaphas_generator::generate(&source, &relative) {
        Ok(items) => items,
        Err(problems) => {
            return problems
                .into_iter()
                .map(|problem| {
                    syn::Error::new(path.span(), problem).into_compile_error()
                })
                .collect::<proc_macro2::TokenStream>()
                .into();
        }
    };
    let documentation = (!attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("doc")))
    .then(|| {
        let documentation =
            format!(" The parser and types of the grammar in `{relative}`.");
        quote! { #[doc = #documentation] }
    });
    // Including the grammar's bytes makes Cargo rebuild when it changes.
    let tracked = file.to_string_lossy();
    quote! {
        #documentation
        #(#attributes)*
        #visibility mod #name {
            const _: &[u8] = ::core::include_bytes!(#tracked);

            #items
        }
    }
    .into()
}
