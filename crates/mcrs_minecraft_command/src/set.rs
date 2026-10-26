use crate::argument::coordinates::{RotationArgument, Vec3Argument};
use crate::argument::entity::EntityArgument;
use crate::argument::keyword::{EntityAnchorArgument, GameModeArgument};
use crate::argument::resource::{
    DimensionArgument, ResourceLocationArgument, ResourceOrTagArgument,
};
use crate::error::{ArgumentError, CommandSyntaxError};
use crate::source::{Lookup, Source};
use mcrs_command::{
    Argument, ArgumentSet, BoolArgument, Contains, DoubleArgument, FloatArgument, IntegerArgument,
    LongArgument, Reader, StringArgument, SuggestionBuilder,
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
    ;
    fn suggest<'t>(
        &'t self,
        source: &'t Source<'_, L>,
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
            VanillaArgument::Integer(_)
            | VanillaArgument::Long(_)
            | VanillaArgument::Float(_)
            | VanillaArgument::Double(_)
            | VanillaArgument::String(_)
            | VanillaArgument::Rotation(_)
            | VanillaArgument::ResourceLocation(_) => {}
        }
    }
}

fn range<T>((min, max): (Option<T>, Option<T>)) -> NumberRange<T> {
    NumberRange { min, max }
}

fn parser_id(key: StaticResourceLocation) -> VarInt {
    let index = parser::ENTRIES
        .iter()
        .position(|&entry| entry == key)
        .unwrap_or_default();
    VarInt(i32::try_from(index).unwrap_or_default())
}

impl VanillaArgument {
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
        };
        ArgumentParser {
            id: parser_id(key),
            properties,
        }
    }
}
