use crate::text::Text;
use crate::{Decode, Encode, Error, Result, VarInt};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_keys::command_argument_type as parser;
use std::borrow::Cow;
use std::io::Write;

const KIND_MASK: u8 = 0x03;
const EXECUTABLE: u8 = 0x04;
const REDIRECT: u8 = 0x08;
const SUGGESTIONS: u8 = 0x10;
const RESTRICTED: u8 = 0x20;

#[derive(Clone, Debug, PartialEq)]
pub struct CommandNode<'a> {
    pub kind: CommandNodeKind<'a>,
    pub children: Vec<VarInt>,
    pub redirect: Option<VarInt>,
    pub executable: bool,
    pub restricted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CommandNodeKind<'a> {
    Root,
    Literal(Cow<'a, str>),
    Argument {
        name: Cow<'a, str>,
        parser: ArgumentParser<'a>,
        suggestions: Option<ResourceLocation<Cow<'a, str>>>,
    },
}

/// A parser id of `minecraft:command_argument_type` and the properties its type reads.
#[derive(Clone, Debug, PartialEq)]
pub struct ArgumentParser<'a> {
    pub id: VarInt,
    pub properties: ParserProperties<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberRange<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StringMode {
    Word,
    Phrase,
    Greedy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParserProperties<'a> {
    None,
    Float(NumberRange<f32>),
    Double(NumberRange<f64>),
    Integer(NumberRange<i32>),
    Long(NumberRange<i64>),
    String(StringMode),
    Entity {
        single: bool,
        players_only: bool,
    },
    Registry(ResourceLocation<Cow<'a, str>>),
    /// `minecraft:time`: the minimum, in ticks, always present.
    Time(i32),
}

impl<T: Encode> Encode for NumberRange<T> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let flags = u8::from(self.min.is_some()) | (u8::from(self.max.is_some()) << 1);
        flags.encode(&mut w)?;
        if let Some(min) = &self.min {
            min.encode(&mut w)?;
        }
        if let Some(max) = &self.max {
            max.encode(&mut w)?;
        }
        Ok(())
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for NumberRange<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let flags = u8::decode(r)?;
        let min = (flags & 0x01 != 0).then(|| T::decode(r)).transpose()?;
        let max = (flags & 0x02 != 0).then(|| T::decode(r)).transpose()?;
        Ok(Self { min, max })
    }
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
            ParserProperties::Registry(registry) => registry.encode(w),
            ParserProperties::Time(min) => min.encode(w),
        }
    }
}

fn decode_properties<'a>(id: VarInt, r: &mut &'a [u8]) -> Result<ParserProperties<'a>> {
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
    } else if is(parser::RESOURCE_OR_TAG) || is(parser::RESOURCE_OR_TAG_KEY) || is(parser::RESOURCE)
    {
        ParserProperties::Registry(Decode::decode(r)?)
    } else if [
        parser::BRIGADIER_BOOL,
        parser::VEC3,
        parser::ROTATION,
        parser::RESOURCE_LOCATION,
        parser::ENTITY_ANCHOR,
        parser::DIMENSION,
        parser::GAMEMODE,
        parser::BLOCK_POS,
        parser::SWIZZLE,
        parser::HEIGHTMAP,
        parser::BLOCK_PREDICATE,
    ]
    .into_iter()
    .any(is)
    {
        ParserProperties::None
    } else {
        return Err(Error::invalid(format!(
            "the properties of parser {key} are not specified"
        )));
    };
    Ok(properties)
}

impl Encode for CommandNode<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let (kind, suggestions) = match &self.kind {
            CommandNodeKind::Root => (0, false),
            CommandNodeKind::Literal(_) => (1, false),
            CommandNodeKind::Argument { suggestions, .. } => (2, suggestions.is_some()),
        };
        let flags = kind
            | if self.executable { EXECUTABLE } else { 0 }
            | if self.redirect.is_some() { REDIRECT } else { 0 }
            | if suggestions { SUGGESTIONS } else { 0 }
            | if self.restricted { RESTRICTED } else { 0 };
        flags.encode(&mut w)?;
        self.children.encode(&mut w)?;
        if let Some(redirect) = self.redirect {
            redirect.encode(&mut w)?;
        }
        match &self.kind {
            CommandNodeKind::Root => {}
            CommandNodeKind::Literal(name) => name.as_ref().encode(&mut w)?,
            CommandNodeKind::Argument {
                name,
                parser,
                suggestions,
            } => {
                name.as_ref().encode(&mut w)?;
                parser.id.encode(&mut w)?;
                parser.properties.encode(&mut w)?;
                if let Some(suggestions) = suggestions {
                    suggestions.encode(&mut w)?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> Decode<'a> for CommandNode<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let flags = u8::decode(r)?;
        let children = Vec::decode(r)?;
        let redirect = (flags & REDIRECT != 0)
            .then(|| VarInt::decode(r))
            .transpose()?;
        let kind = match flags & KIND_MASK {
            0 => CommandNodeKind::Root,
            1 => CommandNodeKind::Literal(Cow::decode(r)?),
            2 => {
                let name = Cow::decode(r)?;
                let id = VarInt::decode(r)?;
                let properties = decode_properties(id, r)?;
                let suggestions = (flags & SUGGESTIONS != 0)
                    .then(|| ResourceLocation::decode(r))
                    .transpose()?;
                CommandNodeKind::Argument {
                    name,
                    parser: ArgumentParser { id, properties },
                    suggestions,
                }
            }
            other => return Err(Error::invalid(format!("node kind {other} does not exist"))),
        };
        Ok(Self {
            kind,
            children,
            redirect,
            executable: flags & EXECUTABLE != 0,
            restricted: flags & RESTRICTED != 0,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct CommandSuggestion<'a> {
    pub text: Cow<'a, str>,
    pub tooltip: Option<Text>,
}
