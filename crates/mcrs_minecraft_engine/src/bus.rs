use bevy_ecs::entity::Entity;
use bevy_ecs::event::Event;
use bevy_ecs::message::Message;
use bevy_math::{DVec3, Vec2};
use bytes::Bytes;
use mcrs_command::Continuation;
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_core::{BlockPos, ColumnPos, ResourceKey};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_network::event::Trigger;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::chunk::{ChunkDataBlockEntity, LightData};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundAddEntity, ClientboundBlockUpdate, ClientboundChunkBatchFinished,
    ClientboundChunkBatchStart, ClientboundChunkCacheRadius, ClientboundCommandSuggestions,
    ClientboundCommands, ClientboundEntityEvent, ClientboundEntityPositionSync,
    ClientboundForgetLevelChunk, ClientboundGameEvent, ClientboundLightUpdate, ClientboundLogin,
    ClientboundPlayerAbilities, ClientboundPlayerLookAt, ClientboundPlayerPosition,
    ClientboundRemoveEntities, ClientboundSetChunkCacheCenter, ClientboundSystemChatPacket,
    LookAnchor,
};
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::uuid::Uuid;
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
    pub dimensions: Arc<[ResourceKey<mcrs_minecraft_dimension::Dimension>]>,
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

/// A command reported success with a message for the operators (CMDP-EXEC-004). A dimension
/// writes it, and the host, which knows every online player and the operators, delivers it to
/// each operator but `source`.
#[derive(Message, Event, Clone, Debug)]
pub struct OperatorBroadcast {
    /// The host anchor of the player that ran the command.
    pub source: Entity,
    pub message: Text,
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
    OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(anchor),
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
    Commands(ClientboundCommands<'static>),
    CommandSuggestions(ClientboundCommandSuggestions<'static>),
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

/// Persistent-only player state snapshot used by cross-dim transfer.
///
/// Current shape carries the minimal viable fields (uuid + username +
/// position + rotation). The full transfer contract (advancements,
/// statistics, inventory, health, game_mode, experience) requires types
/// owned by `MinecraftEntityPlugin`, which remains host-side; pulling
/// those types into this module is out of scope for now.
#[derive(Clone, Debug)]
pub struct PlayerTransferSnapshot {
    pub uuid: Uuid,
    pub username: String,
    pub position: DVec3,
    pub rotation: Vec2,
    pub view_distance: u8,
}

/// Generic move payload.  The `Player` variant carries only the minimal
/// identity the target dim needs to spawn a fresh default entity and relink
/// the connection.  `NonPlayer` carries a kind tag; both slots are shaped so
/// a future faithful-snapshot field is an obvious addition rather than a
/// structural rewrite.
///
/// `PlayerTransferSnapshot` is NOT reused here — it carries position/rotation
/// which arrival resolution overrides, so it is the wrong shape for this role.
#[derive(Clone, Debug)]
pub enum MovePayload {
    Player {
        uuid: Uuid,
        username: String,
    },
    NonPlayer {
        kind: mcrs_minecraft_entity::keys::EntityType,
    },
}

/// Positional and contextual input the target dim needs to resolve the arrival
/// landing point.  The MainWorld forwards this opaquely — only the target
/// DimWorld pattern-matches on it.  The carried position is an input to
/// resolution, not the authoritative spawn position.
#[derive(Clone, Debug)]
pub enum ArrivalCause {
    /// Player crossed a nether portal; target resolves a /8-scaled landing.
    NetherPortal { source_pos: BlockPos },
    /// Player arrived at the End; target creates a 5×5 obsidian platform.
    EndPlatform,
    /// Exact-position respawn (e.g. /tp or death respawn at a fixed point).
    ExactRespawn { pos: DVec3 },
    /// Command teleport (/tp player dim x y z).
    CommandTeleport { pos: DVec3 },
}

/// Forwarded from `ToDim::SpawnEntity` into the target sub-app message bus.
/// The arrival systems read this to spawn the incoming entity and resolve its
/// landing position.
#[derive(Message, Clone, Debug)]
pub struct InboundEntitySpawn {
    pub move_id: mcrs_minecraft_level::session::MoveId,
    pub epoch: u32,
    pub cause: ArrivalCause,
    pub payload: MovePayload,
    pub player: Option<mcrs_minecraft_level::session::PlayerSession>,
}

/// Forwarded from `ToDim::ConfirmMove` into the source sub-app message bus.
/// The source-dim confirm system despawns the hidden in-transit entity.
#[derive(Message, Clone, Debug)]
pub struct InboundConfirmMove {
    pub move_id: mcrs_minecraft_level::session::MoveId,
}

/// Forwarded from `ToDim::RollbackMove` into the source sub-app message bus.
/// The source-dim rollback system removes `InTransit` so the entity reappears.
#[derive(Message, Clone, Debug)]
pub struct InboundRollbackMove {
    pub move_id: mcrs_minecraft_level::session::MoveId,
}

/// The rest of a command line whose sources leave for the dimension `target`. `output` is the
/// host anchor of the player who sent the line, which receives the messages wherever it is.
#[derive(Message, Clone, Debug)]
pub struct CommandHandoff {
    pub target: ResourceKey<Dimension>,
    pub output: Entity,
    pub line: String,
    /// The coordinate scale of the dimension the sources leave; the target scales x and z by
    /// this value over its own (CMD-EX-021).
    pub coordinate_scale: f64,
    pub rest: Continuation<HandoffSource>,
}

/// A source of a command that crosses dimensions. Its entity stays behind: an entity belongs to
/// the world of its dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandoffSource {
    pub level: CommandLevel,
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub anchor: LookAnchor,
}
