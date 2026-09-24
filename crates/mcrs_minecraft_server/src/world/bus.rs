use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_math::{DVec3, Vec2};
use bytes::Bytes;
use mcrs_minecraft_core::{BlockPos, ColumnPos};
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::chunk::{ChunkDataBlockEntity, LightData};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundAddEntity, ClientboundBlockDestruction, ClientboundBlockUpdate,
    ClientboundChunkBatchFinished, ClientboundChunkBatchStart, ClientboundChunkCacheRadius,
    ClientboundContainerClose, ClientboundContainerSetContent, ClientboundContainerSetSlot,
    ClientboundEntityEvent, ClientboundEntityPositionSync, ClientboundForgetLevelChunk,
    ClientboundGameEvent, ClientboundLightUpdate, ClientboundLogin, ClientboundOpenScreen,
    ClientboundPlayerPosition, ClientboundRemoveEntities, ClientboundSetChunkCacheCenter,
    ClientboundSetCursorItem, ClientboundSetEntityData, ClientboundSetEquipment,
    ClientboundSetHeldSlot, ClientboundSetPassengers, ClientboundSystemChatPacket,
    ClientboundTakeItemEntity, ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol::uuid::Uuid;
use smallvec::SmallVec;
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
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerSpawn {
    pub host_anchor: Entity,
    pub session: PlayerSession,
    pub snapshot: PlayerTransferSnapshot,
    /// Dimension resource-location strings forwarded from the host's
    /// `LoadedWorldPreset` so the per-dim spawn consumer can fill
    /// `ClientboundLogin.dimensions` without reading the preset resource
    /// (which is host-only and absent from any DimWorld).
    pub dimensions: Vec<String>,
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

pub(crate) fn to(anchor: Entity, data: PacketPayload) -> OutboundPlayerPacket {
    OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(anchor),
        priority: PacketPriority::Normal,
        data,
        session: PlayerSession(0),
        epoch: 0,
    }
}

impl OutboundPlayerPacket {
    pub(crate) fn critical(self) -> Self {
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
    SetEntityData(ClientboundSetEntityData<'static>),
    SetEquipment(ClientboundSetEquipment),
    UpdateAttributes(ClientboundUpdateAttributes<'static>),
    SetPassengers(ClientboundSetPassengers),
    PlayerLeftView(ClientboundRemoveEntities),
    EntityPosSync(ClientboundEntityPositionSync),
    PlayerLogin(ClientboundLogin<'static>),
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
    SystemChat(ClientboundSystemChatPacket),
    BlockDestruction(ClientboundBlockDestruction),
    GameEvent(ClientboundGameEvent),
    ContainerSetContent(ClientboundContainerSetContent),
    ContainerSetSlot(ClientboundContainerSetSlot),
    SetCursorItem(ClientboundSetCursorItem),
    SetHeldSlot(ClientboundSetHeldSlot),
    OpenScreen(ClientboundOpenScreen),
    ContainerClose(ClientboundContainerClose),
    TakeItemEntity(ClientboundTakeItemEntity),
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
        kind: &'static mcrs_minecraft_world::entity::EntityType,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholder_entity() -> Entity {
        Entity::PLACEHOLDER
    }

    #[test]
    fn bus_message_types_derive_clone_and_debug() {
        let e = placeholder_entity();
        let snapshot = PlayerTransferSnapshot {
            uuid: Uuid::nil(),
            username: "test".to_string(),
            position: DVec3::ZERO,
            rotation: Vec2::ZERO,
            view_distance: 12,
        };

        let outbound = OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(e),
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload::default()),
            session: PlayerSession(0),
            epoch: 0,
        };
        assert_eq!(format!("{:?}", outbound.clone()), format!("{:?}", outbound));

        let inbound = InboundPlayerPacket {
            player: e,
            id: 0,
            data: Bytes::new(),
            timestamp: std::time::Instant::now(),
        };
        assert_eq!(format!("{:?}", inbound.clone()), format!("{:?}", inbound));

        let spawn = InboundPlayerSpawn {
            host_anchor: e,
            session: PlayerSession(0),
            snapshot: snapshot.clone(),
            dimensions: Vec::new(),
        };
        assert_eq!(format!("{:?}", spawn.clone()), format!("{:?}", spawn));

        let attached = OutboundPlayerAttached { host_anchor: e };
        assert_eq!(format!("{:?}", attached.clone()), format!("{:?}", attached));

        let disconnect = OutboundPlayerDisconnect { host_anchor: e };
        assert_eq!(
            format!("{:?}", disconnect.clone()),
            format!("{:?}", disconnect)
        );

        let despawn = InboundPlayerDespawn {
            host_anchor: e,
            session: PlayerSession(0),
        };
        assert_eq!(format!("{:?}", despawn.clone()), format!("{:?}", despawn));
    }

    #[test]
    fn packet_target_player_set_holds_eight_inline() {
        let e = placeholder_entity();
        let mut buf: SmallVec<[Entity; 8]> = SmallVec::new();
        for _ in 0..8 {
            buf.push(e);
        }
        let target = PacketTarget::PlayerSet(buf);
        match &target {
            PacketTarget::PlayerSet(v) => {
                assert_eq!(v.len(), 8);
                assert!(!v.spilled(), "8 entries should fit inline");
            }
            _ => panic!("expected PlayerSet"),
        }
    }
}
