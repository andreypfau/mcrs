use crate::parser_properties::ParserProperties;
use mcrs_minecraft_wire::VarInt;

/// A parser id of `minecraft:command_argument_type` and the properties its type reads.
#[derive(Clone, Debug, PartialEq)]
pub struct ArgumentParser<'a> {
    pub id: VarInt,
    pub properties: ParserProperties<'a>,
}
