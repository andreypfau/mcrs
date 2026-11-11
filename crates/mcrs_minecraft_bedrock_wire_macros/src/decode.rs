use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Lifetime;

use crate::shape::{EnumShape, Shape, StructShape};

pub(crate) fn expand_decode(shape: &Shape) -> TokenStream {
    match shape {
        Shape::Struct(shape) => expand_struct(shape),
        Shape::Enum(shape) => expand_enum(shape),
    }
}

fn expand_struct(shape: &StructShape) -> TokenStream {
    let name = &shape.name;
    let (lifetime, ty_generics) = match &shape.lifetime {
        Some(lifetime) => (lifetime.clone(), Some(quote!(<#lifetime>))),
        None => (input_lifetime(), None),
    };
    let fields = shape.fields.iter().map(|field| {
        let field_name = &field.name;
        let ty = &field.ty;
        match &field.count {
            Some(count) => quote! {
                #field_name: ::mcrs_minecraft_bedrock_wire::decode_counted_list::<#count, _>(r)?,
            },
            None => quote! {
                #field_name: <#ty as ::mcrs_minecraft_bedrock_wire::BedrockDecode<#lifetime>>::decode(r)?,
            },
        }
    });
    quote! {
        impl<#lifetime> ::mcrs_minecraft_bedrock_wire::BedrockDecode<#lifetime> for #name #ty_generics {
            fn decode(
                r: &mut &#lifetime [u8],
            ) -> ::core::result::Result<Self, ::mcrs_minecraft_bedrock_wire::Error> {
                ::core::result::Result::Ok(Self { #(#fields)* })
            }
        }
    }
}

fn expand_enum(shape: &EnumShape) -> TokenStream {
    let name = &shape.name;
    let tag = &shape.tag;
    let variants = &shape.variants;
    let lifetime = input_lifetime();
    quote! {
        impl<#lifetime> ::mcrs_minecraft_bedrock_wire::BedrockDecode<#lifetime> for #name {
            fn decode(
                r: &mut &#lifetime [u8],
            ) -> ::core::result::Result<Self, ::mcrs_minecraft_bedrock_wire::Error> {
                let tag = <#tag as ::mcrs_minecraft_bedrock_wire::BedrockDecode<#lifetime>>::decode(r)?;
                let value = <i64 as ::core::convert::From<#tag>>::from(tag);
                #(
                    if value == Self::#variants as i64 {
                        return ::core::result::Result::Ok(Self::#variants);
                    }
                )*
                ::core::result::Result::Err(::mcrs_minecraft_bedrock_wire::Error::UnknownEnumValue(value))
            }
        }
    }
}

fn input_lifetime() -> Lifetime {
    Lifetime::new("'__input", Span::call_site())
}
