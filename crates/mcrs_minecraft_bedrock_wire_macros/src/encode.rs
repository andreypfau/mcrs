use proc_macro2::TokenStream;
use quote::quote;

use crate::shape::{EnumShape, Shape, StructShape};

pub(crate) fn expand_encode(shape: &Shape) -> TokenStream {
    match shape {
        Shape::Struct(shape) => expand_struct(shape),
        Shape::Enum(shape) => expand_enum(shape),
    }
}

fn expand_struct(shape: &StructShape) -> TokenStream {
    let name = &shape.name;
    let generics = shape.lifetime.as_ref().map(|lifetime| quote!(<#lifetime>));
    let fields = shape.fields.iter().map(|field| {
        let field_name = &field.name;
        match &field.count {
            Some(count) => quote! {
                ::mcrs_minecraft_bedrock_wire::encode_counted_list::<#count, _>(&self.#field_name, w)?;
            },
            None => quote! {
                self.#field_name.encode(w)?;
            },
        }
    });
    let packet = shape.id.as_ref().map(|id| {
        quote! {
            impl #generics ::mcrs_minecraft_bedrock_wire::Packet for #name #generics {
                const ID: u16 = #id;
            }
        }
    });
    quote! {
        impl #generics ::mcrs_minecraft_bedrock_wire::BedrockEncode for #name #generics {
            fn encode(
                &self,
                w: &mut ::mcrs_minecraft_bedrock_wire::__private::Vec<u8>,
            ) -> ::core::result::Result<(), ::mcrs_minecraft_bedrock_wire::Error> {
                use ::mcrs_minecraft_bedrock_wire::BedrockEncode as _;
                #(#fields)*
                ::core::result::Result::Ok(())
            }
        }
        #packet
    }
}

fn expand_enum(shape: &EnumShape) -> TokenStream {
    let name = &shape.name;
    let tag = &shape.tag;
    let variants = &shape.variants;
    quote! {
        impl ::mcrs_minecraft_bedrock_wire::BedrockEncode for #name {
            fn encode(
                &self,
                w: &mut ::mcrs_minecraft_bedrock_wire::__private::Vec<u8>,
            ) -> ::core::result::Result<(), ::mcrs_minecraft_bedrock_wire::Error> {
                let value: i64 = match self {
                    #(Self::#variants => Self::#variants as i64,)*
                };
                let tag = <#tag as ::core::convert::TryFrom<i64>>::try_from(value)
                    .map_err(|_| ::mcrs_minecraft_bedrock_wire::Error::UnknownEnumValue(value))?;
                ::mcrs_minecraft_bedrock_wire::BedrockEncode::encode(&tag, w)
            }
        }
    }
}
