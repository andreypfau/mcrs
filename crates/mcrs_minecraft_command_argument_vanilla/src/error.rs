use core::fmt;
use mcrs_command::SyntaxError;
use mcrs_minecraft_command::{ErrorText, ResourceError, snbt_text};
use mcrs_minecraft_command_argument::CoordinatesError;
use mcrs_minecraft_command_argument::KeywordError;
use mcrs_minecraft_command_argument::TimeError;
use mcrs_minecraft_command_argument_block::BlockError;
use mcrs_minecraft_command_argument_entity::EntityError;
use mcrs_minecraft_nbt_path::NbtPathError;
use mcrs_minecraft_snbt::SnbtError;
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum ArgumentError {
    Resource(ResourceError),
    Keyword(KeywordError),
    Coordinates(CoordinatesError),
    Entity(EntityError),
    Block(BlockError),
    Time(TimeError),
    Nbt(SnbtError),
    NbtPath(NbtPathError),
}

impl fmt::Display for ArgumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgumentError::Resource(error) => fmt::Display::fmt(error, f),
            ArgumentError::Keyword(error) => fmt::Display::fmt(error, f),
            ArgumentError::Coordinates(error) => fmt::Display::fmt(error, f),
            ArgumentError::Entity(error) => fmt::Display::fmt(error, f),
            ArgumentError::Block(error) => fmt::Display::fmt(error, f),
            ArgumentError::Time(error) => fmt::Display::fmt(error, f),
            ArgumentError::Nbt(error) => fmt::Display::fmt(error, f),
            ArgumentError::NbtPath(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl core::error::Error for ArgumentError {}

pub type CommandSyntaxError = SyntaxError<ArgumentError>;

impl From<core::convert::Infallible> for ArgumentError {
    fn from(never: core::convert::Infallible) -> Self {
        match never {}
    }
}

impl From<ResourceError> for ArgumentError {
    fn from(error: ResourceError) -> Self {
        ArgumentError::Resource(error)
    }
}

impl From<KeywordError> for ArgumentError {
    fn from(error: KeywordError) -> Self {
        ArgumentError::Keyword(error)
    }
}

impl From<CoordinatesError> for ArgumentError {
    fn from(error: CoordinatesError) -> Self {
        ArgumentError::Coordinates(error)
    }
}

impl From<EntityError> for ArgumentError {
    fn from(error: EntityError) -> Self {
        match error {
            EntityError::Resource(error) => ArgumentError::Resource(error),
            error => ArgumentError::Entity(error),
        }
    }
}

impl From<BlockError> for ArgumentError {
    fn from(error: BlockError) -> Self {
        match error {
            BlockError::Resource(error) => ArgumentError::Resource(error),
            error => ArgumentError::Block(error),
        }
    }
}

impl From<SnbtError> for ArgumentError {
    fn from(error: SnbtError) -> Self {
        ArgumentError::Nbt(error)
    }
}

impl From<NbtPathError> for ArgumentError {
    fn from(error: NbtPathError) -> Self {
        ArgumentError::NbtPath(error)
    }
}

impl From<TimeError> for ArgumentError {
    fn from(error: TimeError) -> Self {
        ArgumentError::Time(error)
    }
}

impl ErrorText for ArgumentError {
    fn text(&self, input: &str) -> Text {
        match self {
            ArgumentError::Resource(error) => error.text(input),
            ArgumentError::Keyword(error) => error.text(input),
            ArgumentError::Coordinates(error) => error.text(input),
            ArgumentError::Entity(error) => error.text(input),
            ArgumentError::Block(error) => error.text(input),
            ArgumentError::Time(error) => error.text(input),
            ArgumentError::Nbt(error) => snbt_text(input, *error),
            ArgumentError::NbtPath(error) => error.to_string().into_text(),
        }
    }
}
