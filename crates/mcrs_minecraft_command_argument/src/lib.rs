//! The resource, keyword, coordinate, rotation, swizzle and time arguments.

// std: mcrs_minecraft_core needs it.
mod align;
mod block_floor;
mod block_pos_argument;
mod coordinate_reader;
mod coordinates;
mod coordinates_error;
mod keyword;
mod keyword_error;
mod resource;
mod rotation_argument;
mod rotation_toward;
mod rotation_value;
mod swizzle_argument;
mod time;
mod time_error;
mod time_lookup;
mod vec3_argument;
mod world_coordinate;

pub use align::align;
pub use block_floor::block_floor;
pub use block_pos_argument::BlockPosArgument;
pub use coordinates::Coordinates;
pub use coordinates_error::CoordinatesError;
pub use keyword::{EntityAnchorArgument, GameModeArgument, HeightmapArgument, by_name};
pub use keyword_error::KeywordError;
pub use resource::{
    DimensionArgument, ResourceArgument, ResourceLocationArgument, ResourceOrTag,
    ResourceOrTagArgument,
};
pub use rotation_argument::RotationArgument;
pub use rotation_toward::rotation_toward;
pub use rotation_value::RotationValue;
pub use swizzle_argument::SwizzleArgument;
pub use time::{TimeArgument, TimeMarkerArgument, TimelineArgument};
pub use time_error::TimeError;
pub use time_lookup::TimeLookup;
pub use vec3_argument::Vec3Argument;
pub use world_coordinate::WorldCoordinate;
