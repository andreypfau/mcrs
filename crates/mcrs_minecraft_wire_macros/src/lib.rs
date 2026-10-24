#![expect(
    clippy::unwrap_used,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
#![doc = include_str!("../README.md")]
#![deny(
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    rustdoc::missing_crate_level_docs,
    rustdoc::invalid_codeblock_attributes,
    rustdoc::invalid_rust_codeblocks,
    rustdoc::bare_urls,
    rustdoc::invalid_html_tags
)]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_import_braces,
    unreachable_pub,
    clippy::dbg_macro
)]

use proc_macro::TokenStream as StdTokenStream;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{
    Attribute, Data, Fields, GenericParam, Generics, Lifetime, LifetimeParam, LitInt, Result,
    Variant, parse_quote,
};

mod decode;
mod encode;

#[proc_macro_derive(Encode, attributes(packet))]
pub fn derive_encode(item: StdTokenStream) -> StdTokenStream {
    match encode::derive_encode(item.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

#[proc_macro_derive(Decode, attributes(packet))]
pub fn derive_decode(item: StdTokenStream) -> StdTokenStream {
    match decode::derive_decode(item.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

fn pair_variants_with_discriminants(
    variants: impl IntoIterator<Item = Variant>,
) -> Result<Vec<(i32, Variant)>> {
    let mut discriminant = 0;
    variants
        .into_iter()
        .map(|v| {
            if let Some(i) = parse_tag_attr(&v.attrs)? {
                discriminant = i;
            }

            let pair = (discriminant, v);
            discriminant += 1;
            Ok(pair)
        })
        .collect::<Result<_>>()
}

fn parse_tag_attr(attrs: &[Attribute]) -> Result<Option<i32>> {
    for attr in attrs {
        if attr.path().is_ident("packet") {
            let mut res = 0;

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("tag") {
                    res = meta.value()?.parse::<LitInt>()?.base10_parse::<i32>()?;
                    Ok(())
                } else {
                    Err(meta.error("unrecognized argument"))
                }
            })?;

            return Ok(Some(res));
        }
    }

    Ok(None)
}

/// Adding our lifetime to the generics before calling `.split_for_impl()` would
/// also add it to the resulting ty_generics, which we don't want. So I'm doing
/// this hack.
fn decode_split_for_impl(
    mut generics: Generics,
    lifetime: Lifetime,
) -> (TokenStream, TokenStream, TokenStream) {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut impl_generics = impl_generics.to_token_stream();
    let ty_generics = ty_generics.to_token_stream();
    let where_clause = where_clause.to_token_stream();

    if generics.lifetimes().next().is_none() {
        generics
            .params
            .push(GenericParam::Lifetime(LifetimeParam::new(lifetime)));

        impl_generics = generics.split_for_impl().0.to_token_stream();
    }

    (impl_generics, ty_generics, where_clause)
}

/// Bounds the type of every field rather than every type parameter, since a parameter may only
/// select the form of a field, such as a registry reference that is a number on the wire.
fn add_trait_bounds(generics: &mut Generics, data: &Data, trait_: TokenStream) {
    if generics.type_params().next().is_none() {
        return;
    }
    let fields: Vec<&Fields> = match data {
        Data::Struct(s) => vec![&s.fields],
        Data::Enum(e) => e.variants.iter().map(|v| &v.fields).collect(),
        Data::Union(_) => return,
    };
    let mut seen = std::collections::HashSet::new();
    let where_clause = generics.make_where_clause();
    for field in fields.into_iter().flatten() {
        let ty = &field.ty;
        if seen.insert(ty.to_token_stream().to_string()) {
            where_clause.predicates.push(parse_quote!(#ty: #trait_));
        }
    }
}
