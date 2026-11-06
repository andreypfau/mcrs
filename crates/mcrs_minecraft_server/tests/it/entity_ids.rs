use std::collections::BTreeSet;

use bevy_ecs::entity::Entity;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine::entity_ids::{ENTITY_ID_RANGE_LEN, GlobalEntityId};
use mcrs_minecraft_engine::minecraft_entity::MinecraftEntity;
use mcrs_minecraft_replication::wire_id;

use crate::host_app::{self, PlayServer};

fn overworld_and_end() -> (PlayServer, Entity, Entity) {
    let server = PlayServer::start_in(
        host_app::make_host_app(),
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        &[],
        0,
    );
    let overworld = server.dim;
    let end = host_app::label_of(&server.app, "minecraft:the_end");
    (server, overworld, end)
}

fn spawn_entity(server: &mut PlayServer, dim: Entity) -> EntityId {
    let world = server.world_in(dim);
    let entity = world.spawn(MinecraftEntity).id();
    world
        .get::<GlobalEntityId>(entity)
        .expect("a new entity gets a global id")
        .0
}

#[test]
fn an_entity_taken_into_another_dimension_keeps_its_id_and_wire_id() {
    let (mut server, overworld, end) = overworld_and_end();
    let world = server.world_in(overworld);
    let mover = world.spawn(MinecraftEntity).id();
    let carried = *world.get::<GlobalEntityId>(mover).unwrap();
    world.despawn(mover);

    let arrived = server.world_in(end).spawn((MinecraftEntity, carried)).id();
    let kept = *server.world_in(end).get::<GlobalEntityId>(arrived).unwrap();

    assert_eq!(kept, carried);
    assert_eq!(wire_id(kept.0).0 % 2, 0, "player wire ids are odd");
    assert_ne!(spawn_entity(&mut server, end), carried.0);
}

#[test]
fn a_dimension_that_spends_its_range_gets_another_one() {
    let (mut server, overworld, end) = overworld_and_end();
    let mut ids = BTreeSet::new();
    for _ in 0..ENTITY_ID_RANGE_LEN {
        ids.insert(spawn_entity(&mut server, overworld));
    }
    // The output of the next tick asks for a range, and the input of the tick after it grants it.
    server.ticks_all(2);
    for _ in 0..ENTITY_ID_RANGE_LEN {
        ids.insert(spawn_entity(&mut server, overworld));
    }
    ids.insert(spawn_entity(&mut server, end));

    let spawned = usize::try_from(2 * ENTITY_ID_RANGE_LEN + 1).unwrap();
    assert_eq!(ids.len(), spawned, "no two entities share an id");
}
