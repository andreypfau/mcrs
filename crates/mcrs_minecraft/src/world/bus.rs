use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::resource::Resource;
use bevy_math::{DVec3, Vec2};
use bytes::Bytes;
use mcrs_engine::geometry::{BlockPos, ColumnPos};
use mcrs_protocol::BlockStateId;
use mcrs_protocol::chunk::LightData;
use mcrs_protocol::uuid::Uuid;
use mcrs_protocol::{GameMode, Look, Text};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::time::Instant;

use crate::world::entity::player::player_action::PlayerWillDestroyBlock;

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerPacket {
    pub target: PacketTarget,
    pub priority: PacketPriority,
    pub data: PacketPayload,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerPacket {
    pub player: Entity,
    pub id: i32,
    pub data: Bytes,
    pub timestamp: Instant,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerTransfer {
    pub host_anchor: Entity,
    pub dest_dim: Entity,
    pub snapshot: PlayerTransferSnapshot,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerSpawn {
    pub host_anchor: Entity,
    pub snapshot: PlayerTransferSnapshot,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerAttached {
    pub host_anchor: Entity,
    pub new_in_dim_entity: Entity,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerDisconnect {
    pub host_anchor: Entity,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerDespawn {
    pub host_anchor: Entity,
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

#[derive(Clone, Debug)]
pub enum PacketPayload {
    /// Carries all fields ClientboundLightUpdate requires so dispatch_encode
    /// needs no World access: column coordinates plus the owned light payload.
    LightUpdate {
        column: ColumnPos,
        light_data: LightData<'static>,
    },
    Test(TestPayload),
    BlockUpdate {
        position: BlockPos,
        new_state: BlockStateId,
    },
    /// Carries owned chunk bytes and light data so dispatch_encode can build
    /// ClientboundLevelChunkWithLight without World access. The per-dim chunk
    /// producer encodes sections into `chunk_bytes`; dispatch constructs the
    /// borrowing ChunkData at encode time.
    ChunkLoad {
        column: ColumnPos,
        chunk_bytes: Vec<u8>,
        light_data: LightData<'static>,
    },
    ChunkUnload {
        column: ColumnPos,
    },
    /// Carries the wire numeric entity id and all fields ClientboundAddEntity
    /// needs so dispatch_encode needs no World access. The producer resolves
    /// `entity.index_u32() as i32` before emitting this variant.
    PlayerEnteredView {
        entity_id: i32,
        uuid: Uuid,
        kind: i32,
        position: DVec3,
        yaw: f32,
        pitch: f32,
    },
    /// Carries the wire numeric entity id list so dispatch_encode can build
    /// ClientboundRemoveEntities without World access.
    PlayerLeftView {
        entity_ids: SmallVec<[i32; 4]>,
    },
    /// Carries the wire numeric entity id and all fields
    /// ClientboundEntityPositionSync needs so dispatch_encode needs no World
    /// access. The producer resolves `entity.index_u32() as i32`.
    EntityPosSync {
        entity_id: i32,
        position: DVec3,
        velocity: DVec3,
        look: Look,
        on_ground: bool,
    },
    /// Carries all fields ClientboundLogin requires as self-contained owned
    /// wire data so dispatch_encode needs no World access. The per-dim play-
    /// login emitter fills these from the InboundPlayerSpawn snapshot and the
    /// world preset dimensions list.
    PlayerLogin {
        player_id: i32,
        hardcore: bool,
        game_mode: GameMode,
        /// Dimension resource-location strings (e.g. "minecraft:overworld").
        dimensions: Vec<String>,
        max_players: i32,
        chunk_radius: i32,
        simulation_distance: i32,
        reduced_debug_info: bool,
        show_death_screen: bool,
        do_limited_crafting: bool,
        enforces_secure_chat: bool,
    },
    /// Carries the `ClientboundGameEvent { LevelChunksLoadStart }` wire data.
    /// Emitted immediately after `PlayerLogin` during the join sequence.
    LevelChunksLoadStart,
    /// Carries the entity-event data for the op-level status effect sent
    /// during the join sequence (ClientboundEntityEvent).
    PlayerLoginEntityEvent {
        entity_id: i32,
        entity_status: i8,
    },
    /// Sets the client's chunk-load origin. A vanilla 26.1.2 client will not
    /// render any chunks until this packet is received.
    SetChunkCacheCenter {
        x: i32,
        z: i32,
    },
    /// Sets the client's view distance radius.
    SetChunkCacheRadius {
        radius: i32,
    },
    /// Carries owned per-entry data for ClientboundPlayerInfoUpdate so
    /// dispatch_encode needs no World access.
    PlayerInfoUpdate {
        entries: Vec<PlayerInfoEntry>,
    },
    /// Carries the fields ClientboundPlayerPosition (teleport-sync) requires.
    /// Emitted once per join immediately after `PlayerLogin` so the client
    /// renders at the correct spawn position rather than (0,0,0).
    PlayerPosition {
        teleport_id: i32,
        position: DVec3,
    },
    /// Carries an owned system-chat message so dispatch_encode builds
    /// ClientboundSystemChatPacket without World access. The per-dim chat
    /// broadcaster emits this through the bridge instead of writing
    /// `ServerSideConnection` directly (host-resident, never in a sub-app).
    SystemChat {
        content: Text,
        overlay: bool,
    },
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestInboundPayload {
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
}

/// Per-dim partition of inbound player packets awaiting shuttle into a
/// `DimSubApp`. The map is filled by a main-side partition system before
/// extracts run and drained by each sub-app's extract closure into the
/// sub-world's `Messages<InboundPlayerPacket>`. Keyed by the
/// host-anchor `Entity` (the `label_entity` of the destination
/// `DimSubApp`).
///
/// `per_dim` is `pub` because the extract closure receives `&mut World`
/// (not typed system params) and reaches into the resource directly:
/// `main_world.resource_mut::<PendingInboundPartition>().per_dim
/// .entry(label_entity).or_default()`.
#[derive(Resource, Default)]
pub struct PendingInboundPartition {
    pub per_dim: FxHashMap<Entity, Vec<InboundPlayerPacket>>,
}

/// Per-dim partition of inbound lifecycle messages (spawn + despawn)
/// awaiting shuttle into a `DimSubApp`. Filled by main-side bridge
/// systems (and the disconnect cleanup) before extracts run; drained
/// by each sub-app's extract closure into the sub-world's
/// `Messages<InboundPlayerSpawn>` and `Messages<InboundPlayerDespawn>`
/// buffers. Keyed by the same host-anchor `Entity` as
/// `PendingInboundPartition` so a single `label_entity` lookup serves
/// both partitions.
///
/// Lifecycle messages are routed separately from `InboundPlayerPacket`
/// to keep the partition's value type a `Vec<T>` rather than forcing
/// callers to switch on a message-kind enum. Each bundle is small
/// (one spawn or one despawn per player per transfer), so plain `Vec`
/// is enough — no `SmallVec` is needed.
#[derive(Resource, Default)]
pub struct PendingInboundLifecycle {
    pub per_dim: FxHashMap<Entity, LifecycleBundle>,
}

#[derive(Default)]
pub struct LifecycleBundle {
    pub spawns: Vec<InboundPlayerSpawn>,
    pub despawns: Vec<InboundPlayerDespawn>,
    pub block_events: Vec<PlayerWillDestroyBlock>,
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
        };

        let outbound = OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(e),
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload::default()),
        };
        assert_eq!(format!("{:?}", outbound.clone()), format!("{:?}", outbound));

        let inbound = InboundPlayerPacket {
            player: e,
            id: 0,
            data: Bytes::new(),
            timestamp: std::time::Instant::now(),
        };
        assert_eq!(format!("{:?}", inbound.clone()), format!("{:?}", inbound));

        let transfer = OutboundPlayerTransfer {
            host_anchor: e,
            dest_dim: e,
            snapshot: snapshot.clone(),
        };
        assert_eq!(
            format!("{:?}", transfer.clone()),
            format!("{:?}", transfer)
        );

        let spawn = InboundPlayerSpawn {
            host_anchor: e,
            snapshot: snapshot.clone(),
        };
        assert_eq!(format!("{:?}", spawn.clone()), format!("{:?}", spawn));

        let attached = OutboundPlayerAttached {
            host_anchor: e,
            new_in_dim_entity: e,
        };
        assert_eq!(
            format!("{:?}", attached.clone()),
            format!("{:?}", attached)
        );

        let disconnect = OutboundPlayerDisconnect { host_anchor: e };
        assert_eq!(
            format!("{:?}", disconnect.clone()),
            format!("{:?}", disconnect)
        );

        let despawn = InboundPlayerDespawn { host_anchor: e };
        assert_eq!(
            format!("{:?}", despawn.clone()),
            format!("{:?}", despawn)
        );
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

    #[test]
    fn pending_inbound_partition_default_is_empty() {
        let p = PendingInboundPartition::default();
        assert!(p.per_dim.is_empty());
    }

    #[test]
    fn pending_inbound_partition_entry_creates_default_vec() {
        let mut partition = PendingInboundPartition::default();
        let dim = placeholder_entity();
        partition
            .per_dim
            .entry(dim)
            .or_default()
            .push(InboundPlayerPacket {
                player: placeholder_entity(),
                id: 1,
                data: Bytes::new(),
                timestamp: std::time::Instant::now(),
            });
        assert_eq!(partition.per_dim.get(&dim).map(|v| v.len()), Some(1));
    }

    #[test]
    fn pending_inbound_lifecycle_default_is_empty() {
        let p = PendingInboundLifecycle::default();
        assert!(p.per_dim.is_empty());
    }

    #[test]
    fn lifecycle_bundle_default_has_empty_vecs() {
        let b = LifecycleBundle::default();
        assert!(b.spawns.is_empty());
        assert!(b.despawns.is_empty());
        assert!(b.block_events.is_empty());
    }

    #[test]
    fn lifecycle_bundle_block_events_default_empty() {
        let b = LifecycleBundle::default();
        assert!(b.block_events.is_empty());
    }

    #[test]
    fn lifecycle_bundle_accepts_spawn_and_despawn_pushes() {
        let e = placeholder_entity();
        let snapshot = PlayerTransferSnapshot {
            uuid: Uuid::nil(),
            username: "x".into(),
            position: DVec3::ZERO,
            rotation: Vec2::ZERO,
        };
        let mut b = LifecycleBundle::default();
        b.spawns.push(InboundPlayerSpawn {
            host_anchor: e,
            snapshot,
        });
        b.despawns.push(InboundPlayerDespawn { host_anchor: e });
        b.block_events.push(PlayerWillDestroyBlock {
            player: e,
            chunk: e,
            block_pos: mcrs_engine::world::block::BlockPos::new(0, 0, 0),
            block_state: mcrs_protocol::BlockStateId(0),
        });
        assert_eq!(b.spawns.len(), 1);
        assert_eq!(b.despawns.len(), 1);
        assert_eq!(b.block_events.len(), 1);
    }
}
