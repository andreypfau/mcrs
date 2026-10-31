use crate::error::{ArgumentError, CommandSyntaxError};
use crate::vanilla_lookup::VanillaLookup;
use core::fmt;
use mcrs_command::{
    Argument, ArgumentSet, BoolArgument, Contains, ContextView, DoubleArgument, FloatArgument,
    Graph, IntegerArgument, LongArgument, Reader, StringArgument, SuggestionBuilder, SyntaxError,
};
use mcrs_minecraft_block::keys::BLOCK;
use mcrs_minecraft_command::{PermissionCheck, Source, WireArgument};
use mcrs_minecraft_command_argument::{
    BlockPosArgument, RotationArgument, SwizzleArgument, Vec3Argument,
};
use mcrs_minecraft_command_argument::{
    DimensionArgument, EntityAnchorArgument, GameModeArgument, HeightmapArgument, ResourceArgument,
    ResourceLocationArgument, ResourceOrTagArgument,
};
use mcrs_minecraft_command_argument::{TimeArgument, TimeMarkerArgument, TimelineArgument};
use mcrs_minecraft_command_argument_block::BlockPredicateArgument;
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_tree::{ArgumentParser, NumberRange, ParserProperties, StringMode};
use mcrs_minecraft_keys::command_argument_type as parser;
use mcrs_minecraft_resource::StaticResourceLocation;
use mcrs_minecraft_wire::VarInt;

/// The command tree of the vanilla argument types, with handlers of type `H`.
pub type VanillaGraph<H> = Graph<VanillaArgument, PermissionCheck, H>;

/// The argument types of the vanilla commands.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum VanillaArgument {
    /// `brigadier:bool`.
    Bool(BoolArgument),
    /// `brigadier:integer`.
    Integer(IntegerArgument),
    /// `brigadier:long`.
    Long(LongArgument),
    /// `brigadier:float`.
    Float(FloatArgument),
    /// `brigadier:double`.
    Double(DoubleArgument),
    /// `brigadier:string`.
    String(StringArgument),
    /// `minecraft:entity`.
    Entity(EntityArgument),
    /// `minecraft:vec3`.
    Vec3(Vec3Argument),
    /// `minecraft:rotation`.
    Rotation(RotationArgument),
    /// `minecraft:resource_location`.
    ResourceLocation(ResourceLocationArgument),
    /// `minecraft:entity_anchor`.
    EntityAnchor(EntityAnchorArgument),
    /// `minecraft:dimension`.
    Dimension(DimensionArgument),
    /// `minecraft:gamemode`.
    GameMode(GameModeArgument),
    /// `minecraft:resource_or_tag` or `minecraft:resource_or_tag_key`.
    ResourceOrTag(ResourceOrTagArgument),
    /// `minecraft:block_pos`.
    BlockPos(BlockPosArgument),
    /// `minecraft:swizzle`.
    Swizzle(SwizzleArgument),
    /// `minecraft:heightmap`.
    Heightmap(HeightmapArgument),
    /// `minecraft:block_predicate`.
    BlockPredicate(BlockPredicateArgument),
    /// `minecraft:time`.
    Time(TimeArgument),
    /// `minecraft:resource`.
    Resource(ResourceArgument),
    /// A time marker of a clock.
    TimeMarker(TimeMarkerArgument),
    /// A timeline of a clock.
    Timeline(TimelineArgument),
}

macro_rules! variant_conversions {
    ($($variant:ident($ty:ty))+) => {
        $(
            impl From<$ty> for VanillaArgument {
                fn from(argument: $ty) -> Self {
                    VanillaArgument::$variant(argument)
                }
            }

            impl Contains<$ty> for VanillaArgument {
                fn get(&self) -> Option<&$ty> {
                    match self {
                        VanillaArgument::$variant(argument) => Some(argument),
                        _ => None,
                    }
                }
            }
        )+
    };
}

variant_conversions! {
    Bool(BoolArgument)
    Integer(IntegerArgument)
    Long(LongArgument)
    Float(FloatArgument)
    Double(DoubleArgument)
    String(StringArgument)
    Entity(EntityArgument)
    Vec3(Vec3Argument)
    Rotation(RotationArgument)
    ResourceLocation(ResourceLocationArgument)
    EntityAnchor(EntityAnchorArgument)
    Dimension(DimensionArgument)
    GameMode(GameModeArgument)
    ResourceOrTag(ResourceOrTagArgument)
    BlockPos(BlockPosArgument)
    Swizzle(SwizzleArgument)
    Heightmap(HeightmapArgument)
    BlockPredicate(BlockPredicateArgument)
    Time(TimeArgument)
    Resource(ResourceArgument)
    TimeMarker(TimeMarkerArgument)
    Timeline(TimelineArgument)
}

fn check<S, A: Argument<S>>(
    argument: &A,
    reader: &mut Reader<'_>,
    source: &S,
) -> Result<(), CommandSyntaxError>
where
    ArgumentError: From<A::Error>,
{
    argument
        .parse(reader, source)
        .map(drop)
        .map_err(SyntaxError::map_into)
}

impl<L: VanillaLookup> ArgumentSet<Source<'_, L>> for VanillaArgument {
    type Error = ArgumentError;
    type Tooltip = &'static str;

    fn check(
        &self,
        reader: &mut Reader<'_>,
        source: &Source<'_, L>,
    ) -> Result<(), CommandSyntaxError> {
        match self {
            VanillaArgument::Bool(argument) => check(argument, reader, source),
            VanillaArgument::Integer(argument) => check(argument, reader, source),
            VanillaArgument::Long(argument) => check(argument, reader, source),
            VanillaArgument::Float(argument) => check(argument, reader, source),
            VanillaArgument::Double(argument) => check(argument, reader, source),
            VanillaArgument::String(argument) => check(argument, reader, source),
            VanillaArgument::Entity(argument) => check(argument, reader, source),
            VanillaArgument::Vec3(argument) => check(argument, reader, source),
            VanillaArgument::Rotation(argument) => check(argument, reader, source),
            VanillaArgument::ResourceLocation(argument) => check(argument, reader, source),
            VanillaArgument::EntityAnchor(argument) => check(argument, reader, source),
            VanillaArgument::Dimension(argument) => check(argument, reader, source),
            VanillaArgument::GameMode(argument) => check(argument, reader, source),
            VanillaArgument::ResourceOrTag(argument) => check(argument, reader, source),
            VanillaArgument::BlockPos(argument) => check(argument, reader, source),
            VanillaArgument::Swizzle(argument) => check(argument, reader, source),
            VanillaArgument::Heightmap(argument) => check(argument, reader, source),
            VanillaArgument::BlockPredicate(argument) => check(argument, reader, source),
            VanillaArgument::Time(argument) => check(argument, reader, source),
            VanillaArgument::Resource(argument) => check(argument, reader, source),
            VanillaArgument::TimeMarker(argument) => check(argument, reader, source),
            VanillaArgument::Timeline(argument) => check(argument, reader, source),
        }
    }

    fn suggest<'t>(
        &'t self,
        source: &'t Source<'_, L>,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, &'static str>,
    ) {
        match self {
            VanillaArgument::Bool(_) => BoolArgument::suggest(builder),
            VanillaArgument::Entity(argument) => argument.suggest(source, builder),
            VanillaArgument::Vec3(argument) => argument.suggest(builder),
            VanillaArgument::EntityAnchor(_) => EntityAnchorArgument::suggest(builder),
            VanillaArgument::Dimension(_) => DimensionArgument::suggest(source, builder),
            VanillaArgument::GameMode(_) => GameModeArgument::suggest(builder),
            VanillaArgument::ResourceOrTag(argument) => argument.suggest(source, builder),
            VanillaArgument::BlockPos(_) => BlockPosArgument::suggest(builder),
            VanillaArgument::Heightmap(_) => HeightmapArgument::suggest(builder),
            VanillaArgument::BlockPredicate(_) => BlockPredicateArgument::suggest(source, builder),
            VanillaArgument::Time(_) => TimeArgument::suggest(builder),
            VanillaArgument::Resource(argument) => argument.suggest(source, builder),
            VanillaArgument::TimeMarker(argument) => argument.suggest(source, context, builder),
            VanillaArgument::Timeline(argument) => argument.suggest(source, context, builder),
            VanillaArgument::Swizzle(_)
            | VanillaArgument::Integer(_)
            | VanillaArgument::Long(_)
            | VanillaArgument::Float(_)
            | VanillaArgument::Double(_)
            | VanillaArgument::String(_)
            | VanillaArgument::Rotation(_)
            | VanillaArgument::ResourceLocation(_) => {}
        }
    }
}

/// The suggestion type of the `commands` packet.
const ASK_SERVER: StaticResourceLocation = mcrs_minecraft_resource::rl!("minecraft:ask_server");

fn range<T>((min, max): (Option<T>, Option<T>)) -> NumberRange<T> {
    NumberRange { min, max }
}

fn limits<T: Copy>(range: &NumberRange<T>, any: (T, T)) -> (T, T) {
    (range.min.unwrap_or(any.0), range.max.unwrap_or(any.1))
}

/// The arguments with no properties on the wire, each told apart by its parser id alone.
const UNIT_ARGUMENTS: [VanillaArgument; 10] = [
    VanillaArgument::Vec3(Vec3Argument),
    VanillaArgument::Rotation(RotationArgument),
    VanillaArgument::ResourceLocation(ResourceLocationArgument),
    VanillaArgument::EntityAnchor(EntityAnchorArgument),
    VanillaArgument::Dimension(DimensionArgument),
    VanillaArgument::GameMode(GameModeArgument),
    VanillaArgument::BlockPos(BlockPosArgument),
    VanillaArgument::Swizzle(SwizzleArgument),
    VanillaArgument::Heightmap(HeightmapArgument),
    VanillaArgument::BlockPredicate(BlockPredicateArgument),
];

/// The parser of an argument has no id in the `command_argument_type` registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownParser(pub StaticResourceLocation);

impl fmt::Display for UnknownParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the argument parser {} has no registry id",
            self.0.as_str()
        )
    }
}

impl core::error::Error for UnknownParser {}

fn parser_id(key: StaticResourceLocation) -> Result<VarInt, UnknownParser> {
    parser::ENTRIES
        .iter()
        .position(|&entry| entry == key)
        .and_then(|index| i32::try_from(index).ok())
        .map(VarInt)
        .ok_or(UnknownParser(key))
}

impl VanillaArgument {
    /// The registry whose entries the argument reads, if it reads one.
    pub(crate) fn registry(&self) -> Option<StaticResourceLocation> {
        match self {
            VanillaArgument::ResourceOrTag(argument) => Some(argument.registry),
            VanillaArgument::Resource(argument) => Some(argument.registry),
            VanillaArgument::Timeline(_) => Some(TimelineArgument::RESOURCE.registry),
            VanillaArgument::BlockPredicate(_) => Some(BLOCK.location()),
            _ => None,
        }
    }

    /// The inverse of [`WireArgument::wire`], or `None` when no argument has that parser, or
    /// `registry` does not resolve its registry. A time marker and a timeline read back as the
    /// resource location and the resource they are sent as: their clock is not on the wire.
    pub(crate) fn from_wire(
        parser: &ArgumentParser<'_>,
        registry: impl Fn(&str) -> Option<StaticResourceLocation>,
    ) -> Option<Self> {
        let key = usize::try_from(parser.id.0)
            .ok()
            .and_then(|index| parser::ENTRIES.get(index))
            .copied()?;
        let argument = match (key, &parser.properties) {
            (key, ParserProperties::None) if key == parser::BRIGADIER_BOOL => BoolArgument.into(),
            (key, ParserProperties::Integer(range)) if key == parser::BRIGADIER_INTEGER => {
                let (min, max) =
                    limits(range, (IntegerArgument::ANY.min, IntegerArgument::ANY.max));
                IntegerArgument { min, max }.into()
            }
            (key, ParserProperties::Long(range)) if key == parser::BRIGADIER_LONG => {
                let (min, max) = limits(range, (LongArgument::ANY.min, LongArgument::ANY.max));
                LongArgument { min, max }.into()
            }
            (key, ParserProperties::Float(range)) if key == parser::BRIGADIER_FLOAT => {
                let (min, max) = limits(range, (FloatArgument::ANY.min, FloatArgument::ANY.max));
                FloatArgument { min, max }.into()
            }
            (key, ParserProperties::Double(range)) if key == parser::BRIGADIER_DOUBLE => {
                let (min, max) = limits(range, (DoubleArgument::ANY.min, DoubleArgument::ANY.max));
                DoubleArgument { min, max }.into()
            }
            (key, ParserProperties::String(mode)) if key == parser::BRIGADIER_STRING => {
                match mode {
                    StringMode::Word => StringArgument::Word,
                    StringMode::Phrase => StringArgument::Phrase,
                    StringMode::Greedy => StringArgument::Greedy,
                }
                .into()
            }
            (
                key,
                &ParserProperties::Entity {
                    single,
                    players_only,
                },
            ) if key == parser::ENTITY => EntityArgument {
                single,
                players_only,
            }
            .into(),
            (key, ParserProperties::Registry(name))
                if key == parser::RESOURCE_OR_TAG || key == parser::RESOURCE_OR_TAG_KEY =>
            {
                ResourceOrTagArgument {
                    registry: registry(name.as_str())?,
                    must_exist: key == parser::RESOURCE_OR_TAG,
                }
                .into()
            }
            (key, ParserProperties::Registry(name)) if key == parser::RESOURCE => {
                ResourceArgument {
                    registry: registry(name.as_str())?,
                }
                .into()
            }
            (key, &ParserProperties::Time(min)) if key == parser::TIME => {
                TimeArgument { min }.into()
            }
            (_, ParserProperties::None) => UNIT_ARGUMENTS
                .into_iter()
                .find(|argument| argument.wire().is_ok_and(|wire| wire.id == parser.id))?,
            _ => return None,
        };
        Some(argument)
    }
}

impl WireArgument for VanillaArgument {
    type WireError = UnknownParser;

    /// `minecraft:ask_server` for an argument whose suggestions only the server can make.
    fn suggestion_type(&self) -> Option<StaticResourceLocation> {
        matches!(
            self,
            VanillaArgument::TimeMarker(_) | VanillaArgument::Timeline(_)
        )
        .then_some(ASK_SERVER)
    }

    fn wire(&self) -> Result<ArgumentParser<'static>, UnknownParser> {
        let registry =
            |registry: StaticResourceLocation| ParserProperties::Registry(registry.into());
        let (key, properties) = match *self {
            VanillaArgument::Bool(_) => (parser::BRIGADIER_BOOL, ParserProperties::None),
            VanillaArgument::Integer(argument) => (
                parser::BRIGADIER_INTEGER,
                ParserProperties::Integer(range(argument.limits())),
            ),
            VanillaArgument::Long(argument) => (
                parser::BRIGADIER_LONG,
                ParserProperties::Long(range(argument.limits())),
            ),
            VanillaArgument::Float(argument) => (
                parser::BRIGADIER_FLOAT,
                ParserProperties::Float(range(argument.limits())),
            ),
            VanillaArgument::Double(argument) => (
                parser::BRIGADIER_DOUBLE,
                ParserProperties::Double(range(argument.limits())),
            ),
            VanillaArgument::String(mode) => (
                parser::BRIGADIER_STRING,
                ParserProperties::String(match mode {
                    StringArgument::Word => StringMode::Word,
                    StringArgument::Phrase => StringMode::Phrase,
                    StringArgument::Greedy => StringMode::Greedy,
                }),
            ),
            VanillaArgument::Entity(argument) => (
                parser::ENTITY,
                ParserProperties::Entity {
                    single: argument.single,
                    players_only: argument.players_only,
                },
            ),
            VanillaArgument::Vec3(_) => (parser::VEC3, ParserProperties::None),
            VanillaArgument::Rotation(_) => (parser::ROTATION, ParserProperties::None),
            VanillaArgument::ResourceLocation(_) => {
                (parser::RESOURCE_LOCATION, ParserProperties::None)
            }
            VanillaArgument::EntityAnchor(_) => (parser::ENTITY_ANCHOR, ParserProperties::None),
            VanillaArgument::Dimension(_) => (parser::DIMENSION, ParserProperties::None),
            VanillaArgument::GameMode(_) => (parser::GAMEMODE, ParserProperties::None),
            VanillaArgument::ResourceOrTag(argument) => (
                if argument.must_exist {
                    parser::RESOURCE_OR_TAG
                } else {
                    parser::RESOURCE_OR_TAG_KEY
                },
                registry(argument.registry),
            ),
            VanillaArgument::BlockPos(_) => (parser::BLOCK_POS, ParserProperties::None),
            VanillaArgument::Swizzle(_) => (parser::SWIZZLE, ParserProperties::None),
            VanillaArgument::Heightmap(_) => (parser::HEIGHTMAP, ParserProperties::None),
            VanillaArgument::BlockPredicate(_) => (parser::BLOCK_PREDICATE, ParserProperties::None),
            VanillaArgument::Time(argument) => (parser::TIME, ParserProperties::Time(argument.min)),
            VanillaArgument::Resource(argument) => (parser::RESOURCE, registry(argument.registry)),
            VanillaArgument::TimeMarker(_) => (parser::RESOURCE_LOCATION, ParserProperties::None),
            VanillaArgument::Timeline(_) => (
                parser::RESOURCE,
                registry(TimelineArgument::RESOURCE.registry),
            ),
        };
        Ok(ArgumentParser {
            id: parser_id(key)?,
            properties,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::collections::BTreeMap;

    const ITEM: StaticResourceLocation = mcrs_minecraft_resource::rl!("minecraft:item");

    #[derive(Deserialize)]
    struct Registries {
        #[serde(rename = "minecraft:command_argument_type")]
        command_argument_type: Registry,
    }

    #[derive(Deserialize)]
    struct Registry {
        entries: BTreeMap<String, Entry>,
    }

    #[derive(Deserialize)]
    struct Entry {
        protocol_id: i32,
    }

    fn reported() -> BTreeMap<String, i32> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        );
        let text = std::fs::read_to_string(path).unwrap();
        let registries: Registries = serde_json::from_str(&text).unwrap();
        registries
            .command_argument_type
            .entries
            .into_iter()
            .map(|(name, entry)| (name, entry.protocol_id))
            .collect()
    }

    #[test]
    fn the_parser_ids_are_the_protocol_ids_of_the_reported_registry() {
        let reported = reported();
        assert_eq!(parser::ENTRIES.len(), reported.len());
        for (index, entry) in parser::ENTRIES.iter().enumerate() {
            assert_eq!(
                reported.get(entry.as_str()).copied(),
                i32::try_from(index).ok(),
                "{}",
                entry.as_str()
            );
        }
    }

    #[test]
    fn every_argument_the_server_sends_reads_back_as_itself() {
        let reported = reported();
        let mut arguments: Vec<(VanillaArgument, &str)> = vec![
            (BoolArgument.into(), "brigadier:bool"),
            (
                IntegerArgument {
                    min: 0,
                    max: IntegerArgument::ANY.max,
                }
                .into(),
                "brigadier:integer",
            ),
            (
                LongArgument {
                    min: LongArgument::ANY.min,
                    max: 7,
                }
                .into(),
                "brigadier:long",
            ),
            (
                FloatArgument {
                    min: -1.5,
                    max: 2.5,
                }
                .into(),
                "brigadier:float",
            ),
            (DoubleArgument::ANY.into(), "brigadier:double"),
            (StringArgument::Word.into(), "brigadier:string"),
            (StringArgument::Phrase.into(), "brigadier:string"),
            (StringArgument::Greedy.into(), "brigadier:string"),
            (
                EntityArgument {
                    single: true,
                    players_only: false,
                }
                .into(),
                "minecraft:entity",
            ),
            (
                ResourceOrTagArgument {
                    registry: ITEM,
                    must_exist: true,
                }
                .into(),
                "minecraft:resource_or_tag",
            ),
            (
                ResourceOrTagArgument {
                    registry: ITEM,
                    must_exist: false,
                }
                .into(),
                "minecraft:resource_or_tag_key",
            ),
            (
                ResourceArgument { registry: ITEM }.into(),
                "minecraft:resource",
            ),
            (TimeArgument { min: 1 }.into(), "minecraft:time"),
        ];
        let unit_names = [
            "minecraft:vec3",
            "minecraft:rotation",
            "minecraft:resource_location",
            "minecraft:entity_anchor",
            "minecraft:dimension",
            "minecraft:gamemode",
            "minecraft:block_pos",
            "minecraft:swizzle",
            "minecraft:heightmap",
            "minecraft:block_predicate",
        ];
        arguments.extend(UNIT_ARGUMENTS.into_iter().zip(unit_names));
        for (argument, name) in arguments {
            let wire = argument.wire().expect("every vanilla parser has an id");
            assert_eq!(Some(&wire.id.0), reported.get(name), "{name}");
            let read =
                VanillaArgument::from_wire(&wire, |name| (name == ITEM.as_str()).then_some(ITEM));
            assert_eq!(read, Some(argument));
        }
    }
}
