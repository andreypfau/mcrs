// std: mcrs_minecraft_core and mcrs_minecraft_protocol, which hold the identifiers and the text
// of every argument, need it.
pub mod argument {
    pub mod coordinates;
    pub mod entity;
    pub mod keyword;
    pub mod resource;
}
pub mod error;
pub mod identifier;
pub mod packet;
pub mod permission;
pub mod set;
pub mod source;
pub mod teleport;

pub use argument::coordinates::{RotationArgument, Vec3Argument};
pub use argument::entity::{Candidate, EntityArgument};
pub use argument::keyword::{EntityAnchorArgument, GameModeArgument};
pub use argument::resource::{
    DimensionArgument, ResourceLocationArgument, ResourceOrTag, ResourceOrTagArgument,
};
pub use error::{
    ArgumentError, CommandSyntaxError, IntoCommandSyntaxError, argument_text, error_feedback,
    error_message,
};
pub use identifier::Identifier;
pub use packet::{VanillaGraph, player_tree, suggestion_reply};
pub use permission::{CommandLevel, PermissionCheck};
pub use set::VanillaArgument;
pub use source::{Anchor, Lookup, NoLookup, Source};
