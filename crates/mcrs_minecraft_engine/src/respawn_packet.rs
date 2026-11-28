use mcrs_minecraft_dimension::{Dimension as DimensionKey, DimensionType};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRespawn;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

use crate::bus::PacketPayload;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptPlayerData {
    Nothing,
    AttributesAndEntityData,
}

impl KeptPlayerData {
    const fn bits(self) -> u8 {
        match self {
            Self::Nothing => 0x00,
            Self::AttributesAndEntityData => 0x03,
        }
    }
}

#[must_use]
pub fn respawn_packet(
    dimension_type: Id<DimensionType>,
    dimension: ResourceKeyBuf<DimensionKey>,
    game_mode: GameMode,
    kept: KeptPlayerData,
) -> PacketPayload {
    PacketPayload::Respawn(ClientboundRespawn {
        player_spawn_info: PlayerSpawnInfo::new(dimension_type, dimension, game_mode),
        data_to_keep: kept.bits(),
    })
}
