use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields, Result, parse_quote, parse2};

use crate::{add_trait_bounds, decode_split_for_impl, pair_variants_with_discriminants};

#[expect(
    clippy::too_many_lines,
    reason = "the whole Decode expansion for structs and enums sits in one place"
)]
pub(super) fn derive_decode(item: TokenStream) -> Result<TokenStream> {
    let mut input = parse2::<DeriveInput>(item)?;

    let input_name = input.ident;

    if input.generics.lifetimes().count() > 1 {
        return Err(Error::new(
            input.generics.params.span(),
            "type deriving `Decode` must have no more than one lifetime",
        ));
    }

    // Use the lifetime specified in the type definition or just use `'a` if not
    // present.
    let lifetime = input
        .generics
        .lifetimes()
        .next()
        .map(|l| l.lifetime.clone())
        .unwrap_or_else(|| parse_quote!('a));

    add_trait_bounds(
        &mut input.generics,
        &input.data,
        quote!(::mcrs_minecraft_wire::__private::Decode<#lifetime>),
    );

    match input.data {
        Data::Struct(struct_) => {
            let decode_fields = match struct_.fields {
                Fields::Named(fields) => {
                    let init = fields.named.iter().map(|f| {
                        let name = f.ident.as_ref().unwrap();
                        let ctx = format!("failed to decode field `{name}` in `{input_name}`");
                        quote! {
                            #name: Decode::decode(_r).context(#ctx)?,
                        }
                    });

                    quote! {
                        Self {
                            #(#init)*
                        }
                    }
                }
                Fields::Unnamed(fields) => {
                    let init = (0..fields.unnamed.len())
                        .map(|i| {
                            let ctx = format!("failed to decode field `{i}` in `{input_name}`");
                            quote! {
                                Decode::decode(_r).context(#ctx)?,
                            }
                        })
                        .collect::<TokenStream>();

                    quote! {
                        Self(#init)
                    }
                }
                Fields::Unit => quote!(Self),
            };

            let (impl_generics, ty_generics, where_clause) =
                decode_split_for_impl(input.generics, lifetime.clone());

            Ok(quote! {
                impl #impl_generics ::mcrs_minecraft_wire::__private::Decode<#lifetime> for #input_name #ty_generics
                #where_clause
                {
                    fn decode(_r: &mut &#lifetime [u8]) -> ::mcrs_minecraft_wire::__private::Result<Self> {
                        use ::mcrs_minecraft_wire::__private::{Decode, Context};

                        Ok(#decode_fields)
                    }
                }
            })
        }
        Data::Enum(enum_) => {
            let variants = pair_variants_with_discriminants(enum_.variants)?;

            let decode_arms = variants
                .iter()
                .map(|(disc, variant)| {
                    let name = &variant.ident;

                    match &variant.fields {
                        Fields::Named(fields) => {
                            let fields = fields
                                .named
                                .iter()
                                .map(|f| {
                                    let field = f.ident.as_ref().unwrap();
                                    let ctx = format!(
                                        "failed to decode field `{field}` in variant `{name}` in \
                                         `{input_name}`",
                                    );
                                    quote! {
                                        #field: Decode::decode(_r).context(#ctx)?,
                                    }
                                })
                                .collect::<TokenStream>();

                            quote! {
                                #disc => Ok(Self::#name { #fields }),
                            }
                        }
                        Fields::Unnamed(fields) => {
                            let init = (0..fields.unnamed.len())
                                .map(|i| {
                                    let ctx = format!(
                                        "failed to decode field `{i}` in variant `{name}` in \
                                         `{input_name}`",
                                    );
                                    quote! {
                                        Decode::decode(_r).context(#ctx)?,
                                    }
                                })
                                .collect::<TokenStream>();

                            quote! {
                                #disc => Ok(Self::#name(#init)),
                            }
                        }
                        Fields::Unit => quote!(#disc => Ok(Self::#name),),
                    }
                })
                .collect::<TokenStream>();

            let (impl_generics, ty_generics, where_clause) =
                decode_split_for_impl(input.generics, lifetime.clone());

            Ok(quote! {
                impl #impl_generics ::mcrs_minecraft_wire::__private::Decode<#lifetime> for #input_name #ty_generics
                #where_clause
                {
                    fn decode(_r: &mut &#lifetime [u8]) -> ::mcrs_minecraft_wire::__private::Result<Self> {
                        use ::mcrs_minecraft_wire::__private::{Decode, Context, ErrorKind, VarInt};

                        let ctx = concat!("failed to decode enum discriminant in `", stringify!(#input_name), "`");
                        let disc = VarInt::decode(_r).context(ctx)?.0;
                        match disc {
                            #decode_arms
                            tag => Err(ErrorKind::UnknownTag { ty: stringify!(#input_name), tag }.into()),
                        }
                    }
                }
            })
        }
        Data::Union(u) => Err(Error::new(
            u.union_token.span(),
            "cannot derive `Decode` on unions",
        )),
    }
}
