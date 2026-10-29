use mcrs_command::SyntaxError;
use mcrs_minecraft_command::{ErrorText, KeywordError, ResourceError};
use mcrs_minecraft_command_block::BlockError;
use mcrs_minecraft_command_coordinates::CoordinatesError;
use mcrs_minecraft_command_entity::EntityError;
use mcrs_minecraft_command_time::TimeError;
use mcrs_minecraft_protocol::text::Text;

/// The syntax errors of the vanilla argument types. An error of an identifier or of a resource
/// is [`ArgumentError::Resource`] whichever argument read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgumentError {
    Resource(ResourceError),
    Keyword(KeywordError),
    Coordinates(CoordinatesError),
    Entity(EntityError),
    Block(BlockError),
    Time(TimeError),
}

pub type CommandSyntaxError = SyntaxError<ArgumentError>;

/// A library argument type has no errors of its own.
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
        }
    }
}
