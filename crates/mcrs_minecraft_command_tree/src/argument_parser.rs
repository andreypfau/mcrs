use crate::parser_properties::ParserProperties;
use mcrs_minecraft_wire::VarInt;

#[derive(Clone, Debug, PartialEq)]
pub struct ArgumentParser<'a> {
    pub id: VarInt,
    pub properties: ParserProperties<'a>,
}
