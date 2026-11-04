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
    ClientboundAddEntity, ClientboundBlockUpdate, ClientboundChunkBatchFinished,
    ClientboundChunkBatchStart, ClientboundChunkCacheRadius, ClientboundCommandSuggestions,
    ClientboundEntityEvent, ClientboundEntityPositionSync, ClientboundForgetLevelChunk,
    ClientboundGameEvent, ClientboundLightUpdate, ClientboundLogin, ClientboundMoveEntityPos,
    ClientboundMoveEntityPosRot, ClientboundMoveEntityRot, ClientboundPlayerAbilities,
    ClientboundPlayerInfoRemove, ClientboundPlayerInfoUpdate, ClientboundPlayerLookAt,
    ClientboundPlayerPosition, ClientboundRemoveEntities, ClientboundRespawn,
    ClientboundRotateHead, ClientboundSetChunkCacheCenter, ClientboundSetTime,
    ClientboundSystemChatPacket, ClientboundTabList,
};
use mcrs_minecraft_protocol_game::player_list::{PlayerListActions, PlayerListEntry};
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_voxel::ColumnPos;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::sync::Arc;

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
    /// Dimension keys forwarded from the host's
    /// `DimensionList` so the per-dim spawn consumer can fill
    /// `ClientboundLogin.dimensions` without reading the list resource
    /// (which is host-only and absent from any DimWorld).
    pub dimensions: Arc<[ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>]>,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerAttached {
    pub session: PlayerSession,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerDisconnect {
    pub host_anchor: Entity,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerDespawn {
    pub session: PlayerSession,
}

/// The round trip of the last keep-alive of a session, which the host measures on the wire.
#[derive(Message, Clone, Copy, Debug)]
pub struct PlayerLatency {
    pub session: PlayerSession,
    /// In milliseconds.
    pub latency: i32,
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
    Low,
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
    Test(TestPayload),
    BlockUpdate(ClientboundBlockUpdate),
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
    SpawnBundle(SpawnBundle),
    RemoveEntities(ClientboundRemoveEntities),
    EntityPosSync(ClientboundEntityPositionSync),
    EntityMove(ClientboundMoveEntityPos),
    EntityMoveRotate(ClientboundMoveEntityPosRot),
    EntityRotate(ClientboundMoveEntityRot),
    HeadRotation(ClientboundRotateHead),
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
    /// A `player_info_update` packet with its id, encoded once for every viewer that receives
    /// it.
    PlayerInfo(Bytes),
    PlayerInfoRemove(ClientboundPlayerInfoRemove),
    PlayerListChange(ListChange),
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
            Self::EntityPosSync(_)
            | Self::EntityMove(_)
            | Self::EntityMoveRotate(_)
            | Self::EntityRotate(_)
            | Self::HeadRotation(_) => PacketKind::EntityMove,
            Self::SpawnBundle(_) => PacketKind::Spawn,
            Self::RemoveEntities(_) => PacketKind::Remove,
            Self::PlayerInfoUpdate { .. }
            | Self::PlayerInfo(_)
            | Self::PlayerInfoRemove(_)
            | Self::PlayerListChange(_)
            | Self::TabList(_) => PacketKind::PlayerInfo,
            Self::ChunkLoad { .. }
            | Self::ChunkUnload(_)
            | Self::ChunkBatchStart(_)
            | Self::ChunkBatchFinished(_)
            | Self::LightUpdate(_) => PacketKind::Chunk,
            _ => PacketKind::Other,
        }
    }
}

/// A change of the entries that a client already has.
#[derive(Clone, Debug)]
pub enum ListChange {
    /// The entries that the player list starts to show, with their latency.
    Listed(Vec<EntryLatency>),
    /// The entries that the player list stops to show.
    Unlisted(Vec<Uuid>),
    Latency(Vec<EntryLatency>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryLatency {
    pub player_uuid: Uuid,
    /// The round trip in milliseconds.
    pub latency: i32,
}

impl ListChange {
    #[must_use]
    pub fn update_packet(&self) -> ClientboundPlayerInfoUpdate<'static> {
        let latency_entry = |entry: &EntryLatency, listed: bool| PlayerListEntry {
            player_uuid: entry.player_uuid,
            listed,
            ping: entry.latency,
            ..Default::default()
        };
        let (actions, entries): (PlayerListActions, Vec<PlayerListEntry<'static>>) = match self {
            Self::Listed(entries) => (
                PlayerListActions::new()
                    .with_update_listed(true)
                    .with_update_latency(true),
                entries
                    .iter()
                    .map(|entry| latency_entry(entry, true))
                    .collect(),
            ),
            Self::Unlisted(uuids) => (
                PlayerListActions::new().with_update_listed(true),
                uuids
                    .iter()
                    .map(|&player_uuid| PlayerListEntry {
                        player_uuid,
                        ..Default::default()
                    })
                    .collect(),
            ),
            Self::Latency(entries) => (
                PlayerListActions::new().with_update_latency(true),
                entries
                    .iter()
                    .map(|entry| latency_entry(entry, false))
                    .collect(),
            ),
        };
        ClientboundPlayerInfoUpdate {
            actions,
            entries: Cow::Owned(entries),
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

/// The packets that make a subject appear on a client, which the client applies in one tick.
#[derive(Clone, Debug)]
pub struct SpawnBundle {
    /// The `player_info_update` packet of the profile entry of a player, with its id, which the
    /// client needs before it creates the player. It does not set the listing, so a new entry
    /// is not listed. `None` when the client has the entry already.
    pub profile: Option<Bytes>,
    pub entity: ClientboundAddEntity,
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
