use proc_macro::TokenStream;

use crate::shape::Shape;

mod decode;
mod encode;
mod shape;

#[proc_macro_derive(BedrockEncode, attributes(bedrock))]
pub fn derive_bedrock_encode(item: TokenStream) -> TokenStream {
    match Shape::parse(item.into()) {
        Ok(shape) => encode::expand_encode(&shape).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[proc_macro_derive(BedrockDecode, attributes(bedrock))]
pub fn derive_bedrock_decode(item: TokenStream) -> TokenStream {
    match Shape::parse(item.into()) {
        Ok(shape) => decode::expand_decode(&shape).into(),
        Err(error) => error.into_compile_error().into(),
    }
}
