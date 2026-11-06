use crate::client_info::ClientInfo;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_math::{DVec3, Vec2};
use bytes::Bytes;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::event::Trigger;
use mcrs_minecraft_network::metrics::PacketKind;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockEvent, ClientboundBlockUpdate, ClientboundChunkBatchFinished,
    ClientboundChunkBatchStart, ClientboundChunkCacheRadius, ClientboundCommandSuggestions,
    ClientboundEntityEvent, ClientboundEntityPositionSync, ClientboundForgetLevelChunk,
    ClientboundGameEvent, ClientboundLightUpdate, ClientboundLogin, ClientboundPlayerAbilities,
    ClientboundPlayerLookAt, ClientboundPlayerPosition, ClientboundRespawn,
    ClientboundSectionBlocksUpdate, ClientboundSetChunkCacheCenter, ClientboundSetTime,
    ClientboundSystemChatPacket, ClientboundTabList,
};
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_voxel::ColumnPos;
use smallvec::SmallVec;
use std::sync::Arc;

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerPacket {
    pub target: PacketTarget,
    pub priority: PacketPriority,
    pub data: PacketPayload,
    // `dim_outputs::deliver` stamps these, not the dimension. PlayerSession(0) is no session's
    // id, so `bridge_outbound` drops an unstamped packet for one player.
    pub session: PlayerSession,
    pub epoch: u32,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerPacket {
    pub session: PlayerSession,
    pub id: i32,
    pub data: Bytes,
    /// Raises the typed event of the frame's row, which a check of the frame found; `None` for a
    /// row without a structure.
    pub trigger: Option<Trigger>,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerSpawn {
    pub session: PlayerSession,
    pub snapshot: PlayerTransferSnapshot,
    /// The keys of the host's `DimensionList`, which no dimension world holds, for
    /// `ClientboundLogin.dimensions`.
    pub dimensions: Arc<[ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>]>,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerAttached {
    pub session: PlayerSession,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerDespawn {
    pub session: PlayerSession,
}

/// The dimension has saved the player an `InboundPlayerDespawn` named and let it go.
#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerReleased {
    pub session: PlayerSession,
}

#[derive(Clone, Debug)]
pub enum PacketTarget {
    SinglePlayer(PlayerSession),
    /// Every session the host places in the dimension with this host label.
    AllInDim(Entity),
    AllPlayers,
    PlayerSet(SmallVec<[PlayerSession; 8]>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketPriority {
    Critical,
    High,
    Normal,
}

pub fn to(session: PlayerSession, data: PacketPayload) -> OutboundPlayerPacket {
    to_target(PacketTarget::SinglePlayer(session), data)
}

/// A packet for `target` at the normal priority, not yet stamped with a session.
pub fn to_target(target: PacketTarget, data: PacketPayload) -> OutboundPlayerPacket {
    OutboundPlayerPacket {
        target,
        priority: PacketPriority::Normal,
        data,
        session: PlayerSession(0),
        epoch: 0,
    }
}

impl OutboundPlayerPacket {
    pub fn critical(self) -> Self {
        Self {
            priority: PacketPriority::Critical,
            ..self
        }
    }
}

#[derive(Clone, Debug)]
pub enum PacketPayload {
    LightUpdate(ClientboundLightUpdate<'static>),
    BlockUpdate(ClientboundBlockUpdate),
    /// The changes of more than one block of one section in one broadcast.
    SectionBlocksUpdate(ClientboundSectionBlocksUpdate<'static>),
    /// A block event that a block took. It is not a change of the column: a client that takes
    /// the column later does not get it.
    BlockEvent(ClientboundBlockEvent),
    /// A `level_chunk_with_light` packet with its id, encoded once for every player that
    /// receives the column in the same drain.
    ChunkLoad {
        column: ColumnPos,
        packet: Bytes,
    },
    ChunkUnload(ClientboundForgetLevelChunk),
    /// Brackets the columns of one send batch, which the client times to answer with the rate
    /// it can take them at.
    ChunkBatchStart(ClientboundChunkBatchStart),
    ChunkBatchFinished(ClientboundChunkBatchFinished),
    /// The entity bytes of one viewer from the muxer: records of packets with their ids, which
    /// other viewers share.
    Replication(Vec<Bytes>),
    EntityPosSync(ClientboundEntityPositionSync),
    PlayerLogin(ClientboundLogin),
    /// Moves the client to another dimension; it keeps its player.
    Respawn(ClientboundRespawn),
    /// The `ClientboundEntityEvent` that tells a client its own operator level.
    OpLevelEntityEvent(ClientboundEntityEvent),
    /// Sets the client's chunk-load origin. A vanilla 26.1.2 client will not
    /// render any chunks until this packet is received.
    SetChunkCacheCenter(ClientboundSetChunkCacheCenter),
    /// Sets the client's view distance radius.
    SetChunkCacheRadius(ClientboundChunkCacheRadius),
    /// Carries owned per-entry data for ClientboundPlayerInfoUpdate so
    /// dispatch_encode needs no World access.
    PlayerInfoUpdate {
        entries: Vec<PlayerInfoEntry>,
    },
    TabList(ClientboundTabList<'static>),
    PlayerPosition(ClientboundPlayerPosition),
    PlayerLookAt(ClientboundPlayerLookAt),
    PlayerAbilities(ClientboundPlayerAbilities),
    SystemChat(ClientboundSystemChatPacket<'static>),
    GameEvent(ClientboundGameEvent),
    /// The `commands` packet with its id, encoded once for every player of one command level.
    Commands(Bytes),
    CommandSuggestions(ClientboundCommandSuggestions<'static>),
    SetTime(ClientboundSetTime),
}

impl PacketPayload {
    pub fn kind(&self) -> PacketKind {
        match self {
            Self::EntityPosSync(_) => PacketKind::EntityMove,
            Self::Replication(_) => PacketKind::Replication,
            Self::PlayerInfoUpdate { .. } | Self::TabList(_) => PacketKind::PlayerInfo,
            Self::ChunkLoad { .. }
            | Self::ChunkUnload(_)
            | Self::ChunkBatchStart(_)
            | Self::ChunkBatchFinished(_)
            | Self::LightUpdate(_) => PacketKind::Chunk,
            _ => PacketKind::Other,
        }
    }
}

/// Owned player-list entry for use inside `PacketPayload::PlayerInfoUpdate`.
/// Carries the fields needed for the AddPlayer + UpdateGameMode + UpdateListed
/// action combination used during join.
#[derive(Clone, Debug)]
pub struct PlayerInfoEntry {
    pub player_uuid: Uuid,
    pub username: String,
    pub game_mode: GameMode,
    pub listed: bool,
}

/// The state of a joining player that the host hands to the dimension that spawns it.
#[derive(Clone, Debug)]
pub struct PlayerTransferSnapshot {
    pub uuid: Uuid,
    pub username: String,
    pub position: DVec3,
    pub rotation: Vec2,
    /// What the client said about itself before it joined; absent when it said nothing.
    pub client_info: Option<ClientInfo>,
}
