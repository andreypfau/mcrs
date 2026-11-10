//! Canonical text components as Bedrock formatted strings or `rawtext`, with server-side
//! dynamic parts resolved and the features Bedrock lacks counted.

// std: mcrs_minecraft_lang and mcrs_minecraft_text need std.
mod bedrock_text;
mod drops;
mod formatting;
mod rawtext;
mod resolver;

pub use bedrock_text::{
    BedrockText, has_translate, to_bedrock, to_bedrock_rawtext, to_bedrock_string,
};
pub use drops::BedrockDrops;
pub use rawtext::{BedrockRawtext, BedrockRawtextComponent, BedrockRawtextWith};
pub use resolver::{BedrockServerResolver, DefaultServerResolver};
