use crate::attribute_state::AttributeState;
use crate::delta_base::Pose;
use mcrs_engine_muxer::ViewerId;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_profile::Property;
use uuid::Uuid;

/// The state of a player when it comes into a world.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSpawn {
    /// The viewer whose own subject the player is.
    pub viewer: ViewerId,
    pub uuid: Uuid,
    pub username: String,
    pub properties: Vec<Property>,
    pub pose: Pose,
    pub game_mode: GameMode,
    /// Each syncable instance that the player holds, one for each attribute (TRK-DATA-004).
    pub attributes: Vec<AttributeState>,
}
