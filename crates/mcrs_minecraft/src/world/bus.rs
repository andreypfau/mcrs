use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::resource::Resource;
use bevy_math::{DVec3, Vec2};
use mcrs_protocol::chunk::LightData;
use mcrs_protocol::uuid::Uuid;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerPacket {
    pub target: PacketTarget,
    pub priority: PacketPriority,
    pub data: PacketPayload,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerPacket {
    pub player: Entity,
    pub packet: TestInboundPayload,
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
    LightUpdate(LightData<'static>),
    Test(TestPayload),
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
            packet: TestInboundPayload::default(),
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
                packet: TestInboundPayload { seq: 1 },
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
        assert_eq!(b.spawns.len(), 1);
        assert_eq!(b.despawns.len(), 1);
    }
}
