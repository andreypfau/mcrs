pub mod clientbound {
    use crate::chunk::ChunkBlockUpdateEntry;
    use crate::entity::player::*;
    use crate::game_event::GameEventKind;
    use crate::packets::common::clientbound::KeepAlive;
    use crate::{ChunkColumnPos, Look, PositionFlag, VarInt};
    use bevy_math::DVec3;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_protocol::BlockStateId;
    use mcrs_protocol_macros::{Decode, Encode, Packet};
    use std::borrow::Cow;
    use valence_ident::Ident;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x05, state=Game)]
    pub struct ClientboundBlockDestruction {
        pub id: VarInt,
        pub pos: BlockPos,
        pub progress: i8,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x08, state=Game)]
    pub struct ClientboundBlockUpdate {
        pub block_pos: BlockPos,
        pub block_state_id: BlockStateId,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x2B, state=Game)]
    pub struct ClientboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x30, state=Game)]
    pub struct ClientboundLogin<'a> {
        pub player_id: i32,
        pub hardcore: bool,
        pub dimensions: Vec<Ident<Cow<'a, str>>>,
        pub max_players: VarInt,
        pub chunk_radius: VarInt,
        pub simulation_distance: VarInt,
        pub reduced_debug_info: bool,
        pub show_death_screen: bool,
        pub do_limited_crafting: bool,
        pub player_spawn_info: PlayerSpawnInfo<'a>,
        pub enforces_secure_chat: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x25, state=Game)]
    pub struct ClientboundForgetLevelChunk {
        pub z: i32,
        pub x: i32,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x26, state=Game)]
    pub struct ClientboundGameEvent {
        pub game_event: GameEventKind,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x2C, state=Game)]
    pub struct ClientboundLevelChunkWithLight<'a> {
        pub pos: ChunkColumnPos,
        pub chunk_data: crate::chunk::ChunkData<'a>,
        pub light_data: crate::chunk::LightData<'a>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x46, state=Game)]
    pub struct ClientboundPlayerPosition {
        pub teleport_id: VarInt,
        pub position: DVec3,
        pub velocity: DVec3,
        pub look: Look,
        pub flags: Vec<PositionFlag>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x52, state=Game)]
    pub struct ClientboundSectionBlocksUpdate<'a> {
        pub chunk_pos: ChunkPos,
        pub blocks: Cow<'a, [ChunkBlockUpdateEntry]>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x5C, state=Game)]
    pub struct ClientboundSetChunkCacheCenter {
        pub x: VarInt,
        pub z: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x5D, state=Game)]
    pub struct ClientboundChunkCacheRadius {
        pub radius: VarInt,
    }
}

pub mod serverbound {
    use crate::entity::player::PlayerAction;
    use crate::packets::common::serverbound::{ClientInformation, KeepAlive};
    use crate::pos::MoveFlags;
    use crate::{Direction, Look, Position, VarInt};
    use derive_more::From;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_protocol_macros::{Decode, Encode, Packet};

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x0D, state=Game)]
    pub struct ServerboundClientInformation<'a>(pub ClientInformation<'a>);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1B, state=Game)]
    pub struct ServerboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1D, state=Game)]
    pub struct ServerboundMovePlayerPos {
        pub position: Position,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1E, state=Game)]
    pub struct ServerboundMovePlayerPosRot {
        pub position: Position,
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1F, state=Game)]
    pub struct ServerboundMovePlayerRot {
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x20, state=Game)]
    pub struct ServerboundMovePlayerStatusOnly {
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x28, state=Game)]
    pub struct ServerboundPlayerAction {
        pub action: PlayerAction,
        pub pos: BlockPos,
        pub direction: Direction,
        pub sequence: VarInt,
    }
}
