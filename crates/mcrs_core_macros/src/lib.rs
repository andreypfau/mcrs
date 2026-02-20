use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

/// Validates and normalizes a resource location string at compile time.
///
/// Returns a tuple expression `("namespace:path", colon_pos_u16)`.
///
/// - If no namespace is given (no `:`), assumes `"minecraft:"` prefix.
/// - Validates charset: namespace `[a-z0-9_.-]`, path `[a-z0-9_.-/]`.
/// - Computes `colon_pos` as a `u16` literal.
#[proc_macro]
pub fn rl_impl(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    let raw = lit.value();

    let (namespace, path) = if let Some((ns, p)) = raw.split_once(':') {
        (ns.to_owned(), p.to_owned())
    } else {
        ("minecraft".to_owned(), raw.clone())
    };

    // Validate namespace
    if namespace.is_empty() {
        return syn::Error::new(lit.span(), "resource location namespace must not be empty")
            .into_compile_error()
            .into();
    }
    for (i, c) in namespace.chars().enumerate() {
        if !matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-') {
            return syn::Error::new(
                lit.span(),
                format!(
                    "invalid character '{}' at position {} in namespace \"{}\" \
                     (allowed: a-z 0-9 _ . -)",
                    c, i, namespace
                ),
            )
            .into_compile_error()
            .into();
        }
    }

    // Validate path
    if path.is_empty() {
        return syn::Error::new(lit.span(), "resource location path must not be empty")
            .into_compile_error()
            .into();
    }
    for (i, c) in path.chars().enumerate() {
        if !matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-' | '/') {
            return syn::Error::new(
                lit.span(),
                format!(
                    "invalid character '{}' at position {} in path \"{}\" \
                     (allowed: a-z 0-9 _ . - /)",
                    c, i, path
                ),
            )
            .into_compile_error()
            .into();
        }
    }

    let full = format!("{}:{}", namespace, path);
    let colon_pos = namespace.len() as u16;

    let expanded = quote! {
        (#full, #colon_pos)
    };
    expanded.into()
}
