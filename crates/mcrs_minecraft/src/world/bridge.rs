use bevy_ecs::message::Messages;
use bevy_ecs::system::ResMut;

use crate::world::bus::{InboundPlayerPacket, PendingInboundPartition};
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
}
