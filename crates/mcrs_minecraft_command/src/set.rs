use crate::argument::block::BlockPredicateArgument;
use crate::argument::coordinates::{
    BlockPosArgument, RotationArgument, SwizzleArgument, Vec3Argument,
};
use crate::argument::entity::EntityArgument;
use crate::argument::keyword::{EntityAnchorArgument, GameModeArgument, HeightmapArgument};
use crate::argument::resource::{
    DimensionArgument, ResourceLocationArgument, ResourceOrTagArgument,
};
use crate::argument::time::{ResourceArgument, TimeArgument, TimeMarkerArgument, TimelineArgument};
use crate::error::{ArgumentError, CommandSyntaxError};
use crate::source::{Lookup, Source};
use mcrs_command::{
    Argument, ArgumentSet, BoolArgument, Contains, ContextView, DoubleArgument, FloatArgument,
    IntegerArgument, LongArgument, Reader, StringArgument, SuggestionBuilder,
};
use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_keys::command_argument_type as parser;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::command::{ArgumentParser, NumberRange, ParserProperties, StringMode};

macro_rules! vanilla_arguments {
    ($($variant:ident($ty:ty),)+ ; $($suggest:tt)+) => {
        /// The argument types of the vanilla commands.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub enum VanillaArgument {
            $($variant($ty),)+
        }

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

        impl<L: Lookup> ArgumentSet<Source<'_, L>> for VanillaArgument {
            type Error = ArgumentError;
            type Tooltip = &'static str;

            fn check(
                &self,
                reader: &mut Reader<'_>,
                source: &Source<'_, L>,
            ) -> Result<(), CommandSyntaxError> {
                match self {
                    $(VanillaArgument::$variant(argument) => argument
                        .parse(reader, source)
                        .map(drop)
                        .map_err(mcrs_command::SyntaxError::map_into),)+
                }
            }

            $($suggest)+
        }
    };
}

vanilla_arguments! {
    Bool(BoolArgument),
    Integer(IntegerArgument),
    Long(LongArgument),
    Float(FloatArgument),
    Double(DoubleArgument),
    String(StringArgument),
    Entity(EntityArgument),
    Vec3(Vec3Argument),
    Rotation(RotationArgument),
    ResourceLocation(ResourceLocationArgument),
    EntityAnchor(EntityAnchorArgument),
    Dimension(DimensionArgument),
    GameMode(GameModeArgument),
    ResourceOrTag(ResourceOrTagArgument),
    BlockPos(BlockPosArgument),
    Swizzle(SwizzleArgument),
    Heightmap(HeightmapArgument),
    BlockPredicate(BlockPredicateArgument),
    Time(TimeArgument),
    Resource(ResourceArgument),
    TimeMarker(TimeMarkerArgument),
    Timeline(TimelineArgument),
    ;
    fn suggest<'t>(
        &'t self,
        source: &'t Source<'_, L>,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, &'static str>,
    ) {
        match self {
            VanillaArgument::Bool(_) => BoolArgument::suggest(builder),
            VanillaArgument::Entity(a) => a.suggest(source, builder),
            VanillaArgument::Vec3(a) => a.suggest(builder),
            VanillaArgument::EntityAnchor(_) => EntityAnchorArgument::suggest(builder),
            VanillaArgument::Dimension(_) => DimensionArgument::suggest(source, builder),
            VanillaArgument::GameMode(_) => GameModeArgument::suggest(builder),
            VanillaArgument::ResourceOrTag(a) => a.suggest(source, builder),
            VanillaArgument::BlockPos(_) => BlockPosArgument::suggest(builder),
            VanillaArgument::Heightmap(_) => HeightmapArgument::suggest(builder),
            VanillaArgument::BlockPredicate(_) => BlockPredicateArgument::suggest(source, builder),
            VanillaArgument::Time(_) => TimeArgument::suggest(builder),
            VanillaArgument::Resource(a) => a.suggest(source, builder),
            VanillaArgument::TimeMarker(a) => a.suggest(source, context, builder),
            VanillaArgument::Timeline(a) => a.suggest(source, context, builder),
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

/// The suggestion type of the `commands` packet (CMDP-ARG-002).
const ASK_SERVER: StaticResourceLocation = mcrs_minecraft_core::rl!("minecraft:ask_server");

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

fn parser_id(key: StaticResourceLocation) -> VarInt {
    let index = parser::ENTRIES
        .iter()
        .position(|&entry| entry == key)
        .unwrap_or_default();
    VarInt(i32::try_from(index).unwrap_or_default())
}

impl VanillaArgument {
    /// `minecraft:ask_server` for an argument whose suggestions only the server can make.
    pub(crate) fn suggestion_type(&self) -> Option<StaticResourceLocation> {
        matches!(
            self,
            VanillaArgument::TimeMarker(_) | VanillaArgument::Timeline(_)
        )
        .then_some(ASK_SERVER)
    }

    /// The parser id and the properties of the `commands` packet (CMDP-ARG-001).
    pub(crate) fn wire(&self) -> ArgumentParser<'static> {
        let registry =
            |registry: StaticResourceLocation| ParserProperties::Registry(registry.into());
        let (key, properties) = match *self {
            VanillaArgument::Bool(_) => (parser::BRIGADIER_BOOL, ParserProperties::None),
            VanillaArgument::Integer(a) => (
                parser::BRIGADIER_INTEGER,
                ParserProperties::Integer(range(a.limits())),
            ),
            VanillaArgument::Long(a) => (
                parser::BRIGADIER_LONG,
                ParserProperties::Long(range(a.limits())),
            ),
            VanillaArgument::Float(a) => (
                parser::BRIGADIER_FLOAT,
                ParserProperties::Float(range(a.limits())),
            ),
            VanillaArgument::Double(a) => (
                parser::BRIGADIER_DOUBLE,
                ParserProperties::Double(range(a.limits())),
            ),
            VanillaArgument::String(mode) => (
                parser::BRIGADIER_STRING,
                ParserProperties::String(match mode {
                    StringArgument::Word => StringMode::Word,
                    StringArgument::Phrase => StringMode::Phrase,
                    StringArgument::Greedy => StringMode::Greedy,
                }),
            ),
            VanillaArgument::Entity(a) => (
                parser::ENTITY,
                ParserProperties::Entity {
                    single: a.single,
                    players_only: a.players_only,
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
            VanillaArgument::ResourceOrTag(a) => (
                if a.must_exist {
                    parser::RESOURCE_OR_TAG
                } else {
                    parser::RESOURCE_OR_TAG_KEY
                },
                registry(a.registry),
            ),
            VanillaArgument::BlockPos(_) => (parser::BLOCK_POS, ParserProperties::None),
            VanillaArgument::Swizzle(_) => (parser::SWIZZLE, ParserProperties::None),
            VanillaArgument::Heightmap(_) => (parser::HEIGHTMAP, ParserProperties::None),
            VanillaArgument::BlockPredicate(_) => (parser::BLOCK_PREDICATE, ParserProperties::None),
            VanillaArgument::Time(a) => (parser::TIME, ParserProperties::Time(a.min)),
            VanillaArgument::Resource(a) => (parser::RESOURCE, registry(a.registry)),
            VanillaArgument::TimeMarker(_) => (parser::RESOURCE_LOCATION, ParserProperties::None),
            VanillaArgument::Timeline(a) => (parser::RESOURCE, registry(a.resource.registry)),
        };
        ArgumentParser {
            id: parser_id(key),
            properties,
        }
    }

    /// The inverse of [`Self::wire`], or `None` when no argument has that parser, or
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
            (k, ParserProperties::None) if k == parser::BRIGADIER_BOOL => BoolArgument.into(),
            (k, ParserProperties::Integer(range)) if k == parser::BRIGADIER_INTEGER => {
                let (min, max) =
                    limits(range, (IntegerArgument::ANY.min, IntegerArgument::ANY.max));
                IntegerArgument { min, max }.into()
            }
            (k, ParserProperties::Long(range)) if k == parser::BRIGADIER_LONG => {
                let (min, max) = limits(range, (LongArgument::ANY.min, LongArgument::ANY.max));
                LongArgument { min, max }.into()
            }
            (k, ParserProperties::Float(range)) if k == parser::BRIGADIER_FLOAT => {
                let (min, max) = limits(range, (FloatArgument::ANY.min, FloatArgument::ANY.max));
                FloatArgument { min, max }.into()
            }
            (k, ParserProperties::Double(range)) if k == parser::BRIGADIER_DOUBLE => {
                let (min, max) = limits(range, (DoubleArgument::ANY.min, DoubleArgument::ANY.max));
                DoubleArgument { min, max }.into()
            }
            (k, ParserProperties::String(mode)) if k == parser::BRIGADIER_STRING => match mode {
                StringMode::Word => StringArgument::Word,
                StringMode::Phrase => StringArgument::Phrase,
                StringMode::Greedy => StringArgument::Greedy,
            }
            .into(),
            (
                k,
                &ParserProperties::Entity {
                    single,
                    players_only,
                },
            ) if k == parser::ENTITY => EntityArgument {
                single,
                players_only,
            }
            .into(),
            (k, ParserProperties::Registry(name))
                if k == parser::RESOURCE_OR_TAG || k == parser::RESOURCE_OR_TAG_KEY =>
            {
                ResourceOrTagArgument {
                    registry: registry(name.as_str())?,
                    must_exist: k == parser::RESOURCE_OR_TAG,
                }
                .into()
            }
            (k, ParserProperties::Registry(name)) if k == parser::RESOURCE => ResourceArgument {
                registry: registry(name.as_str())?,
            }
            .into(),
            (k, &ParserProperties::Time(min)) if k == parser::TIME => TimeArgument { min }.into(),
            (_, ParserProperties::None) => UNIT_ARGUMENTS
                .into_iter()
                .find(|argument| argument.wire().id == parser.id)?,
            _ => return None,
        };
        Some(argument)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM: StaticResourceLocation = mcrs_minecraft_core::rl!("minecraft:item");

    #[test]
    fn every_argument_the_server_sends_reads_back_as_itself() {
        let mut arguments: Vec<VanillaArgument> = vec![
            BoolArgument.into(),
            IntegerArgument {
                min: 0,
                max: IntegerArgument::ANY.max,
            }
            .into(),
            LongArgument {
                min: LongArgument::ANY.min,
                max: 7,
            }
            .into(),
            FloatArgument {
                min: -1.5,
                max: 2.5,
            }
            .into(),
            DoubleArgument::ANY.into(),
            StringArgument::Word.into(),
            StringArgument::Phrase.into(),
            StringArgument::Greedy.into(),
            EntityArgument {
                single: true,
                players_only: false,
            }
            .into(),
            ResourceOrTagArgument {
                registry: ITEM,
                must_exist: true,
            }
            .into(),
            ResourceOrTagArgument {
                registry: ITEM,
                must_exist: false,
            }
            .into(),
            ResourceArgument { registry: ITEM }.into(),
            TimeArgument { min: 1 }.into(),
        ];
        arguments.extend(UNIT_ARGUMENTS);
        for argument in arguments {
            let read = VanillaArgument::from_wire(&argument.wire(), |name| {
                (name == ITEM.as_str()).then_some(ITEM)
            });
            assert_eq!(read, Some(argument));
        }
    }
}
