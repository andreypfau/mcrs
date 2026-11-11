//! Derives `BedrockEncode` and `BedrockDecode` of `mcrs_minecraft_bedrock_wire`.
// std: proc_macro

use proc_macro::TokenStream;

use crate::shape::Shape;

mod decode;
mod encode;
mod shape;

/// Derives `BedrockEncode`: the fields in declaration order, or the discriminant of a fieldless
/// enum as its `tag` type. `#[bedrock(id = <expr>)]` on a struct also implements `Packet`.
#[proc_macro_derive(BedrockEncode, attributes(bedrock))]
pub fn derive_bedrock_encode(item: TokenStream) -> TokenStream {
    match Shape::parse(item.into()) {
        Ok(shape) => encode::expand_encode(&shape).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Derives `BedrockDecode`, the inverse of `BedrockEncode`; an unknown enum tag is
/// `Error::UnknownEnumValue`.
#[proc_macro_derive(BedrockDecode, attributes(bedrock))]
pub fn derive_bedrock_decode(item: TokenStream) -> TokenStream {
    match Shape::parse(item.into()) {
        Ok(shape) => decode::expand_decode(&shape).into(),
        Err(error) => error.into_compile_error().into(),
    }
}
