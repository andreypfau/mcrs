use bevy_ecs::message::Messages;
use bevy_ecs::system::ResMut;

use crate::world::bus::{
    InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerTransfer,
    PendingInboundLifecycle, PendingInboundPartition,
};
use crate::world::player_index::PlayerIndex;

pub fn partition_main_inbound(
    mut msgs: ResMut<Messages<InboundPlayerPacket>>,
    mut partition: ResMut<PendingInboundPartition>,
    mut player_index: ResMut<PlayerIndex>,
) {
    for msg in msgs.drain() {
        let Some(location) = player_index.get_mut(&msg.player) else {
            continue;
        };
        if location.in_dim_entity.is_some() {
            partition
                .per_dim
                .entry(location.current_dim)
                .or_default()
                .push(msg);
        } else {
            location.inbound_pending.push(msg);
        }
    }
}

pub fn bridge_player_transfer(
    mut transfer_msgs: ResMut<Messages<OutboundPlayerTransfer>>,
    mut player_index: ResMut<PlayerIndex>,
    mut lifecycle: ResMut<PendingInboundLifecycle>,
) {
    for msg in transfer_msgs.drain() {
        let Some(location) = player_index.get_mut(&msg.host_anchor) else {
            continue;
        };
        location.current_dim = msg.dest_dim;
        location.in_dim_entity = None;
        let spawn = InboundPlayerSpawn {
            host_anchor: msg.host_anchor,
            snapshot: msg.snapshot.clone(),
        };
        lifecycle
            .per_dim
            .entry(msg.dest_dim)
            .or_default()
            .spawns
            .push(spawn);
    }
}

pub fn bridge_player_attach(
    mut attach_msgs: ResMut<Messages<OutboundPlayerAttached>>,
    mut player_index: ResMut<PlayerIndex>,
    mut partition: ResMut<PendingInboundPartition>,
) {
    for msg in attach_msgs.drain() {
        let drained_and_dim = {
            let Some(location) = player_index.get_mut(&msg.host_anchor) else {
                continue;
            };
            location.in_dim_entity = Some(msg.new_in_dim_entity);
            let drained = std::mem::take(&mut location.inbound_pending);
            let current_dim = location.current_dim;
            (drained, current_dim)
        };
        let (drained, current_dim) = drained_and_dim;
        if !drained.is_empty() {
            let bucket = partition.per_dim.entry(current_dim).or_default();
            for packet in drained {
                bucket.push(packet);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;
    use bevy_ecs::system::{IntoSystem, System};
    use bevy_ecs::world::World;
    use smallvec::SmallVec;

    use crate::world::bus::TestInboundPayload;
    use crate::world::player_index::PlayerLocation;

    fn make_location(
        current_dim: Entity,
        in_dim_entity: Option<Entity>,
    ) -> PlayerLocation {
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim,
            previous_dim: None,
            in_dim_entity,
            inbound_pending: SmallVec::new(),
        }
    }

    fn run_partition(world: &mut World) {
        let mut sys = IntoSystem::into_system(partition_main_inbound);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    fn write_inbound(world: &mut World, msg: InboundPlayerPacket) {
        world
            .resource_mut::<Messages<InboundPlayerPacket>>()
            .write(msg);
    }

    #[test]
    fn partition_routes_to_pending_when_in_dim_entity_is_some() {
        let mut world = World::new();
        world.init_resource::<Messages<InboundPlayerPacket>>();
        world.init_resource::<PendingInboundPartition>();
        world.init_resource::<PlayerIndex>();

        let player = Entity::from_raw_u32(100).expect("nonzero");
        let dim = Entity::from_raw_u32(7).expect("nonzero");
        let in_dim = Entity::from_raw_u32(42).expect("nonzero");

        world
            .resource_mut::<PlayerIndex>()
            .insert(player, make_location(dim, Some(in_dim)));
        write_inbound(
            &mut world,
            InboundPlayerPacket {
                player,
                packet: TestInboundPayload { seq: 1 },
            },
        );

        run_partition(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert_eq!(partition.per_dim.get(&dim).map(|v| v.len()), Some(1));

        let index = world.resource::<PlayerIndex>();
        let location = index.get(&player).expect("present");
        assert!(location.inbound_pending.is_empty());
    }

    #[test]
    fn partition_routes_to_inbound_pending_when_in_dim_entity_is_none() {
        let mut world = World::new();
        world.init_resource::<Messages<InboundPlayerPacket>>();
        world.init_resource::<PendingInboundPartition>();
        world.init_resource::<PlayerIndex>();

        let player = Entity::from_raw_u32(100).expect("nonzero");
        let dim = Entity::from_raw_u32(7).expect("nonzero");

        world
            .resource_mut::<PlayerIndex>()
            .insert(player, make_location(dim, None));
        write_inbound(
            &mut world,
            InboundPlayerPacket {
                player,
                packet: TestInboundPayload { seq: 2 },
            },
        );

        run_partition(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert!(partition.per_dim.is_empty());

        let index = world.resource::<PlayerIndex>();
        let location = index.get(&player).expect("present");
        assert_eq!(location.inbound_pending.len(), 1);
    }

    #[test]
    fn partition_drops_unknown_player_silently() {
        let mut world = World::new();
        world.init_resource::<Messages<InboundPlayerPacket>>();
        world.init_resource::<PendingInboundPartition>();
        world.init_resource::<PlayerIndex>();

        let unknown = Entity::from_raw_u32(999).expect("nonzero");
        write_inbound(
            &mut world,
            InboundPlayerPacket {
                player: unknown,
                packet: TestInboundPayload { seq: 3 },
            },
        );

        run_partition(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert!(partition.per_dim.is_empty());

        let msgs = world.resource::<Messages<InboundPlayerPacket>>();
        let mut reader = msgs.get_cursor();
        assert_eq!(reader.read(msgs).count(), 0);
    }

    use crate::world::bus::PlayerTransferSnapshot;
    use bevy_math::{DVec3, Vec2};
    use mcrs_protocol::uuid::Uuid;

    fn synthetic_snapshot() -> PlayerTransferSnapshot {
        PlayerTransferSnapshot {
            uuid: Uuid::nil(),
            username: "test".into(),
            position: DVec3::ZERO,
            rotation: Vec2::ZERO,
        }
    }

    fn run_transfer(world: &mut World) {
        let mut sys = IntoSystem::into_system(bridge_player_transfer);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    fn run_attach(world: &mut World) {
        let mut sys = IntoSystem::into_system(bridge_player_attach);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    #[test]
    fn bridge_player_transfer_updates_player_index_and_writes_spawn() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerTransfer>>();
        world.init_resource::<Messages<InboundPlayerSpawn>>();
        world.init_resource::<PlayerIndex>();
        world.init_resource::<PendingInboundLifecycle>();

        let host_anchor = Entity::from_raw_u32(42).expect("nonzero");
        let src_dim = Entity::from_raw_u32(1).expect("nonzero");
        let dest_dim = Entity::from_raw_u32(2).expect("nonzero");
        let src_in_dim = Entity::from_raw_u32(99).expect("nonzero");

        world.resource_mut::<PlayerIndex>().insert(
            host_anchor,
            PlayerLocation {
                socket: Entity::PLACEHOLDER,
                current_dim: src_dim,
                previous_dim: None,
                in_dim_entity: Some(src_in_dim),
                inbound_pending: SmallVec::new(),
            },
        );

        world
            .resource_mut::<Messages<OutboundPlayerTransfer>>()
            .write(OutboundPlayerTransfer {
                host_anchor,
                dest_dim,
                snapshot: synthetic_snapshot(),
            });

        run_transfer(&mut world);

        let index = world.resource::<PlayerIndex>();
        let loc = index.get(&host_anchor).expect("present");
        assert_eq!(loc.current_dim, dest_dim);
        assert!(loc.in_dim_entity.is_none());

        let lifecycle = world.resource::<PendingInboundLifecycle>();
        let bundle = lifecycle
            .per_dim
            .get(&dest_dim)
            .expect("dest dim bundle present");
        assert_eq!(bundle.spawns.len(), 1);
        assert_eq!(bundle.spawns[0].host_anchor, host_anchor);
    }

    #[test]
    fn bridge_player_attach_sets_in_dim_entity_and_drains_inbound_pending() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerAttached>>();
        world.init_resource::<PlayerIndex>();
        world.init_resource::<PendingInboundPartition>();

        let host_anchor = Entity::from_raw_u32(42).expect("nonzero");
        let dest_dim = Entity::from_raw_u32(2).expect("nonzero");
        let new_in_dim = Entity::from_raw_u32(200).expect("nonzero");

        let mut buffered: SmallVec<[InboundPlayerPacket; 4]> = SmallVec::new();
        for seq in 0..3u32 {
            buffered.push(InboundPlayerPacket {
                player: host_anchor,
                packet: TestInboundPayload { seq },
            });
        }

        world.resource_mut::<PlayerIndex>().insert(
            host_anchor,
            PlayerLocation {
                socket: Entity::PLACEHOLDER,
                current_dim: dest_dim,
                previous_dim: None,
                in_dim_entity: None,
                inbound_pending: buffered,
            },
        );

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached {
                host_anchor,
                new_in_dim_entity: new_in_dim,
            });

        run_attach(&mut world);

        let index = world.resource::<PlayerIndex>();
        let loc = index.get(&host_anchor).expect("present");
        assert_eq!(loc.in_dim_entity, Some(new_in_dim));
        assert!(loc.inbound_pending.is_empty());

        let partition = world.resource::<PendingInboundPartition>();
        let dest_bucket = partition
            .per_dim
            .get(&dest_dim)
            .expect("dest dim bucket present");
        assert_eq!(dest_bucket.len(), 3);
    }

    #[test]
    fn bridge_player_attach_idempotent_on_unknown_host_anchor() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerAttached>>();
        world.init_resource::<PlayerIndex>();
        world.init_resource::<PendingInboundPartition>();

        let unknown = Entity::from_raw_u32(999).expect("nonzero");
        let new_in_dim = Entity::from_raw_u32(1).expect("nonzero");

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached {
                host_anchor: unknown,
                new_in_dim_entity: new_in_dim,
            });

        run_attach(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert!(partition.per_dim.is_empty());
    }
}
