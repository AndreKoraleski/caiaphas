//! The `grammar!` macro of caiaphas. The `caiaphas` crate re-exports it.

use std::{env, fs, path::Path};

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Ident, LitInt, LitStr, Token, Visibility, braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// What `grammar!` is called with: the module to generate, the grammar it
/// parses, and how to generate it.
struct Input {
    attributes: Vec<Attribute>,
    visibility: Visibility,
    name: Ident,
    source: Source,
    /// Whether the suggestions about the grammar stop the build. `suggest`
    /// after the grammar asks for that.
    suggest: bool,
    /// How many matches of recursive rules the parser follows inside one
    /// another, when `depth = N` after the grammar says.
    depth: Option<usize>,
}

/// Where `grammar!` finds the grammar.
enum Source {
    /// The path of a file from the root of the crate, written after `=`.
    File(LitStr),
    /// The grammar itself, written as a string between braces.
    Inline(LitStr),
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attributes = input.call(Attribute::parse_outer)?;
        let visibility = input.parse()?;
        input.parse::<Token![mod]>()?;
        let name = input.parse()?;
        let source = if input.parse::<Option<Token![=]>>()?.is_some() {
            Source::File(input.parse()?)
        } else {
            let body;
            braced!(body in input);
            let grammar = body.parse()?;
            if !body.is_empty() {
                return Err(
                    body.error("expected only the grammar as a string")
                );
            }
            Source::Inline(grammar)
        };
        let (mut suggest, mut depth) = (false, None);
        while input.parse::<Option<Token![,]>>()?.is_some() {
            let flag: Ident = input.parse()?;
            match flag.to_string().as_str() {
                "suggest" => suggest = true,
                "depth" => {
                    input.parse::<Token![=]>()?;
                    let value: LitInt = input.parse()?;
                    let value: usize = value.base10_parse()?;
                    if value == 0 {
                        let message = "a parser needs a depth of at least 1";
                        return Err(syn::Error::new(flag.span(), message));
                    }
                    depth = Some(value);
                }
                _ => {
                    let message = "expected `suggest` or `depth = N`";
                    return Err(syn::Error::new(flag.span(), message));
                }
            }
        }
        input.parse::<Option<Token![;]>>()?;
        Ok(Self {
            attributes,
            visibility,
            name,
            source,
            suggest,
            depth,
        })
    }
}

/// Generates a module that parses a grammar, with a type for each of its
/// rules.
///
/// ```text
/// // A grammar in a file, with a path from the root of the crate.
/// caiaphas::grammar!(pub mod json = "grammars/json.ebnf");
///
/// // A short grammar written in the macro.
/// caiaphas::grammar!(pub mod pair { "pair = 'x', ',', 'y';" });
/// ```
///
/// Attributes and doc comments before `mod` go to the module. `suggest` after
/// the grammar makes every suggestion stop the build, as a warning does.
/// `depth = N` lets the parser follow `N` matches of recursive rules inside
/// one another, instead of 512, for a program with a larger stack. The
/// [tutorial] tells what the module holds.
///
/// [tutorial]: https://docs.rs/caiaphas/latest/caiaphas/tutorial/
#[proc_macro]
pub fn grammar(input: TokenStream) -> TokenStream {
    let Input {
        attributes,
        visibility,
        name,
        source,
        suggest,
        depth,
    } = parse_macro_input!(input as Input);
    // The literal every problem points at, the grammar, the name rendered
    // problems give it, and the file to rebuild after, if any.
    let (literal, grammar, label, file) = match source {
        Source::File(path) => {
            let relative = path.value();
            // A grammar's path is relative to the root of the crate that uses
            // it. The path of `include_str!` is relative to the file instead.
            let root = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let file = Path::new(&root).join(&relative);
            let grammar = match fs::read_to_string(&file) {
                Ok(grammar) => grammar,
                Err(error) => {
                    let message = if file.is_dir() {
                        format!("`{relative}` is a directory, not a grammar")
                    } else {
                        format!(
                            "cannot read `{}`: {error}; the path of a \
                             grammar starts where the crate's Cargo.toml is",
                            file.display()
                        )
                    };
                    return syn::Error::new(path.span(), message)
                        .into_compile_error()
                        .into();
                }
            };
            (path, grammar, relative, Some(file))
        }
        Source::Inline(text) => {
            let grammar = text.value();
            (text, grammar, format!("mod {name}"), None)
        }
    };
    let errors = |problems: Vec<String>| -> TokenStream {
        problems
            .into_iter()
            .map(|problem| {
                syn::Error::new(literal.span(), problem).into_compile_error()
            })
            .collect::<proc_macro2::TokenStream>()
            .into()
    };
    let mut options = caiaphas_generator::Options::default();
    options.depth = depth;
    let module = match caiaphas_generator::generate(&grammar, &label, options)
    {
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
        let documentation = match &file {
            Some(_) => {
                format!(" The parser and types of the grammar in `{label}`.")
            }
            None => " The parser and types of a grammar.".to_owned(),
        };
        quote! { #[doc = #documentation] }
    });
    let suggestions = (!module.suggestions.is_empty()).then(|| {
        let documentation = format!(
            "\n # Suggestions\n\n These changes to the grammar would make its \
             parser faster:\n\n```text\n{}\n```",
            module.suggestions.join("\n")
        );
        quote! { #[doc = #documentation] }
    });
    // Including a grammar file's bytes makes Cargo rebuild when it changes.
    let tracked = file.map(|file| {
        let file = file.to_string_lossy().into_owned();
        quote! { const _: &[u8] = ::core::include_bytes!(#file); }
    });
    // Errors in what the grammar names, such as the type of a conversion,
    // point at the grammar rather than at the whole call.
    let items = located(
        module.items,
        proc_macro2::Span::call_site().located_at(literal.span()),
    );
    quote! {
        #documentation
        #(#attributes)*
        #suggestions
        #visibility mod #name {
            #tracked

            #items
        }
    }
    .into()
}

/// Returns `items` with every token placed at `span`.
fn located(
    items: proc_macro2::TokenStream,
    span: proc_macro2::Span,
) -> proc_macro2::TokenStream {
    items
        .into_iter()
        .map(|mut token| {
            if let proc_macro2::TokenTree::Group(group) = &token {
                let mut inner = proc_macro2::Group::new(
                    group.delimiter(),
                    located(group.stream(), span),
                );
                inner.set_span(span);
                token = proc_macro2::TokenTree::Group(inner);
            }
            token.set_span(span);
            token
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(input: &Input) -> Option<String> {
        match &input.source {
            Source::File(path) => Some(path.value()),
            Source::Inline(_) => None,
        }
    }

    #[test]
    fn reads_the_module_and_the_path() {
        let input: Input =
            syn::parse_str("#[doc = \"x\"] pub mod json = \"json.ebnf\";")
                .unwrap();
        assert_eq!(input.name, "json");
        assert_eq!(file(&input).as_deref(), Some("json.ebnf"));
        assert_eq!(input.attributes.len(), 1);
        assert!(!input.suggest);
    }

    #[test]
    fn reads_a_grammar_written_in_the_macro() {
        let input: Input =
            syn::parse_str("mod pair { \"pair = 'x';\" }, suggest").unwrap();
        let Source::Inline(grammar) = &input.source else {
            panic!("the grammar is inline");
        };
        assert_eq!(grammar.value(), "pair = 'x';");
        assert!(input.suggest);
        assert!(syn::parse_str::<Input>("mod pair { \"a\" \"b\" }").is_err());
    }

    #[test]
    fn reads_whether_to_stop_at_suggestions() {
        let input: Input =
            syn::parse_str("mod json = \"json.ebnf\", suggest").unwrap();
        assert!(input.suggest);
        assert!(syn::parse_str::<Input>("mod json = \"j\", hint").is_err());
        assert!(
            syn::parse_str::<Input>("mod json = \"j\", optimize").is_err()
        );
    }

    #[test]
    fn reads_every_flag_after_the_grammar() {
        let input: Input =
            syn::parse_str("mod json = \"j\", depth = 9, suggest;").unwrap();
        assert!(input.suggest);
        assert_eq!(input.depth, Some(9));
    }
}
