use crate::game_mode::OptGameMode;
use crate::{GameMode, GlobalPos, VarInt};
use std::borrow::Cow;
use valence_ident::{ident, Ident};
use mcrs_protocol_macros::{Decode, Encode};

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct PlayerSpawnInfo<'a> {
    pub dimension_type_id: VarInt,
    pub dimension: Ident<Cow<'a, str>>,
    pub seed: u64,
    pub game_mode: GameMode,
    pub prev_game_mode: OptGameMode,
    pub is_debug: bool,
    pub is_flat: bool,
    pub last_depth_location: Option<GlobalPos<'a>>,
    pub portal_cooldown: VarInt,
    pub sea_level: VarInt,
}

impl Default for PlayerSpawnInfo<'_> {
    fn default() -> Self {
        Self {
            dimension_type_id: VarInt(0),
            dimension: Ident::from(ident!("overworld")),
            seed: 0,
            game_mode: GameMode::Survival,
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
    AbortDestroyBlock,
    StopDestroyBlock,
    DropAllItems,
    DropItem,
    ReleaseUseItem,
    SwapItemWithOffhand,
    Stab
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum HumanoidArm {
    Left,
    Right
}