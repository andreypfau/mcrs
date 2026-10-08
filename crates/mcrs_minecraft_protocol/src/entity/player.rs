use crate::game_mode::OptGameMode;
use crate::{Bounded, GameMode, GlobalPos, VarInt};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_protocol_macros::{Decode, Encode};
use mcrs_minecraft_registry::Id;

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct PlayerSpawnInfo {
    pub dimension_type_id: Id<DimensionType>,
    pub dimension: ResourceKey<Dimension>,
    pub game_mode: GameMode,
    pub prev_game_mode: OptGameMode,
    pub is_debug: bool,
    pub is_flat: bool,
    pub last_depth_location: Option<GlobalPos>,
    pub portal_cooldown: VarInt,
    pub sea_level: VarInt,
}

impl PlayerSpawnInfo {
    pub fn new(
        dimension_type_id: Id<DimensionType>,
        dimension: ResourceKey<Dimension>,
        game_mode: GameMode,
    ) -> Self {
        Self {
            dimension_type_id,
            dimension,
            game_mode,
            prev_game_mode: OptGameMode::default(),
            is_debug: false,
            is_flat: true,
            last_depth_location: None,
            portal_cooldown: VarInt(0),
            sea_level: VarInt(63),
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Encode, Decode)]
pub enum PlayerAction {
    StartDestroyBlock,
    ChangeDestroyDirection,
    AbortDestroyBlock,
    StopDestroyBlock,
    DropAllItems,
    DropItem,
    ReleaseUseItem,
    SwapItemWithOffhand,
    Stab,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum HumanoidArm {
    Left,
    Right,
}

#[derive(Copy, Clone, Debug, Encode, Decode)]
pub struct CommandArgumentSignature<'a> {
    pub argument_name: Bounded<&'a str, 16>,
    pub signature: &'a [u8; 256],
}

#[derive(Copy, Clone, Debug, Encode, Decode)]
pub struct MessageSignature {
    pub offset: VarInt,
    pub acknowledged: [u8; 3],
    pub checksum: u8,
}
