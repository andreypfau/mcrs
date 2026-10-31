use crate::client_info::ClientInfo;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_math::{DVec3, Vec2};
use bytes::Bytes;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::event::Trigger;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::chunk::{ChunkDataBlockEntity, LightData};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundAddEntity, ClientboundBlockUpdate, ClientboundChunkBatchFinished,
    ClientboundChunkBatchStart, ClientboundChunkCacheRadius, ClientboundCommandSuggestions,
    ClientboundEntityEvent, ClientboundEntityPositionSync, ClientboundForgetLevelChunk,
    ClientboundGameEvent, ClientboundLightUpdate, ClientboundLogin, ClientboundPlayerAbilities,
    ClientboundPlayerLookAt, ClientboundPlayerPosition, ClientboundRemoveEntities,
    ClientboundRespawn, ClientboundSetChunkCacheCenter, ClientboundSetTime,
    ClientboundSystemChatPacket,
};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_voxel::ColumnPos;
use smallvec::SmallVec;
use std::sync::Arc;
use std::time::Instant;

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerPacket {
    pub target: PacketTarget,
    pub priority: PacketPriority,
    pub data: PacketPayload,
    // Stamped by the bridge extract closure, not by dim systems.
    // Default PlayerSession(0) / epoch 0 is safe: PlayerSession(0) never
    // is a session's id (counter starts at 1), so unstamped
    // packets are always dropped by bridge_outbound.
    pub session: PlayerSession,
    pub epoch: u32,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerPacket {
    pub player: Entity,
    pub id: i32,
    pub data: Bytes,
    pub timestamp: Instant,
    /// Raises the typed event of the frame's row, which a check of the frame found; `None` for a
    /// row without a structure.
    pub trigger: Option<Trigger>,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerSpawn {
    pub host_anchor: Entity,
    pub session: PlayerSession,
    pub snapshot: PlayerTransferSnapshot,
    /// Dimension keys forwarded from the host's
    /// `DimensionList` so the per-dim spawn consumer can fill
    /// `ClientboundLogin.dimensions` without reading the list resource
    /// (which is host-only and absent from any DimWorld).
    pub dimensions: Arc<[ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>]>,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerAttached {
    pub host_anchor: Entity,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerDisconnect {
    pub host_anchor: Entity,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerDespawn {
    pub host_anchor: Entity,
    pub session: PlayerSession,
}

/// The dimension has saved the player an `InboundPlayerDespawn` named and let it go.
#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerReleased {
    pub session: PlayerSession,
}

#[derive(Clone, Debug)]
pub enum PacketTarget {
    SinglePlayer(Entity),
    AllInDim(Entity),
    AllPlayers,
    PlayerSet(SmallVec<[Entity; 8]>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketPriority {
    Critical,
    High,
    Normal,
    Low,
}

pub fn to(anchor: Entity, data: PacketPayload) -> OutboundPlayerPacket {
    to_target(PacketTarget::SinglePlayer(anchor), data)
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
    Test(TestPayload),
    BlockUpdate(ClientboundBlockUpdate),
    /// Carries owned chunk bytes and light data so dispatch_encode can build
    /// ClientboundLevelChunkWithLight without World access. The per-dim chunk
    /// producer encodes sections into `chunk_bytes`; dispatch constructs the
    /// borrowing ChunkData at encode time.
    ChunkLoad {
        column: ColumnPos,
        chunk_bytes: Vec<u8>,
        heightmaps: Vec<(VarInt, Vec<u64>)>,
        light_data: LightData<'static>,
        block_entities: Vec<ChunkDataBlockEntity<'static>>,
    },
    ChunkUnload(ClientboundForgetLevelChunk),
    /// Brackets the columns of one send batch, which the client times to answer with the rate
    /// it can take them at.
    ChunkBatchStart(ClientboundChunkBatchStart),
    ChunkBatchFinished(ClientboundChunkBatchFinished),
    PlayerEnteredView(ClientboundAddEntity),
    PlayerLeftView(ClientboundRemoveEntities),
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
    PlayerPosition(ClientboundPlayerPosition),
    PlayerLookAt(ClientboundPlayerLookAt),
    PlayerAbilities(ClientboundPlayerAbilities),
    SystemChat(ClientboundSystemChatPacket),
    GameEvent(ClientboundGameEvent),
    /// The `commands` packet with its id, encoded once for every player of one command level.
    Commands(Bytes),
    CommandSuggestions(ClientboundCommandSuggestions<'static>),
    SetTime(ClientboundSetTime),
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestPayload {
    pub seq: u32,
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
