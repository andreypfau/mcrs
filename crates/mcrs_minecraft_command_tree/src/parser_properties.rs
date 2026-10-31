use crate::number_range::NumberRange;
use crate::string_mode::StringMode;
use mcrs_minecraft_keys::command_argument_type as parser;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_wire::{Decode, Encode, Error, Result, VarInt};
use std::io::Write;

/// The properties that follow a parser id on the wire.
#[derive(Clone, Debug, PartialEq)]
pub enum ParserProperties<'a> {
    /// The parser has no properties.
    None,
    /// `brigadier:float`.
    Float(NumberRange<f32>),
    /// `brigadier:double`.
    Double(NumberRange<f64>),
    /// `brigadier:integer`.
    Integer(NumberRange<i32>),
    /// `brigadier:long`.
    Long(NumberRange<i64>),
    /// `brigadier:string`.
    String(StringMode),
    /// `minecraft:entity`: whether it takes one entity and whether only players.
    Entity { single: bool, players_only: bool },
    /// `minecraft:score_holder`: whether the argument takes more than one holder.
    ScoreHolder { multiple: bool },
    /// The registry of a resource or tag argument.
    Registry(ResourceLocation<'a>),
    /// `minecraft:time`: the minimum, in ticks, always present.
    Time(i32),
}

impl Encode for ParserProperties<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        match self {
            ParserProperties::None => Ok(()),
            ParserProperties::Float(range) => range.encode(w),
            ParserProperties::Double(range) => range.encode(w),
            ParserProperties::Integer(range) => range.encode(w),
            ParserProperties::Long(range) => range.encode(w),
            ParserProperties::String(mode) => mode.encode(w),
            ParserProperties::Entity {
                single,
                players_only,
            } => (u8::from(*single) | (u8::from(*players_only) << 1)).encode(&mut w),
            ParserProperties::ScoreHolder { multiple } => u8::from(*multiple).encode(w),
            ParserProperties::Time(min) => min.encode(w),
            ParserProperties::Registry(registry) => registry.encode(w),
        }
    }
}

pub(crate) fn decode_properties<'a>(id: VarInt, r: &mut &'a [u8]) -> Result<ParserProperties<'a>> {
    let key = usize::try_from(id.0)
        .ok()
        .and_then(|index| parser::ENTRIES.get(index))
        .ok_or_else(|| Error::invalid(format!("parser id {} is not in the registry", id.0)))?;
    let is = |entry| *key == entry;
    let properties = if is(parser::BRIGADIER_FLOAT) {
        ParserProperties::Float(Decode::decode(r)?)
    } else if is(parser::BRIGADIER_DOUBLE) {
        ParserProperties::Double(Decode::decode(r)?)
    } else if is(parser::BRIGADIER_INTEGER) {
        ParserProperties::Integer(Decode::decode(r)?)
    } else if is(parser::BRIGADIER_LONG) {
        ParserProperties::Long(Decode::decode(r)?)
    } else if is(parser::BRIGADIER_STRING) {
        ParserProperties::String(Decode::decode(r)?)
    } else if is(parser::ENTITY) {
        let flags = u8::decode(r)?;
        ParserProperties::Entity {
            single: flags & 0x01 != 0,
            players_only: flags & 0x02 != 0,
        }
    } else if is(parser::TIME) {
        ParserProperties::Time(Decode::decode(r)?)
    } else if is(parser::SCORE_HOLDER) {
        ParserProperties::ScoreHolder {
            multiple: u8::decode(r)? & 0x01 != 0,
        }
    } else if [
        parser::RESOURCE_OR_TAG,
        parser::RESOURCE_OR_TAG_KEY,
        parser::RESOURCE,
        parser::RESOURCE_KEY,
        parser::RESOURCE_SELECTOR,
    ]
    .into_iter()
    .any(is)
    {
        ParserProperties::Registry(Decode::decode(r)?)
    } else {
        ParserProperties::None
    };
    Ok(properties)
}
