use bevy_ecs::world::World;
use glam::DVec3;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::ReplicationData;
use mcrs_minecraft_replication::SpawnLook;
use mcrs_minecraft_vanilla_item_entity::Collect;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::Launch;
use mcrs_voxel::BlockPos;

use crate::fixture::DIRT;
use crate::fixture::FLOOR;
use crate::fixture::PLAYER;
use crate::fixture::PLAYER_ID;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::collector;
use crate::fixture::fill_slot;
use crate::fixture::launch;
use crate::fixture::move_collector;
use crate::fixture::run;

/// What the item entities published since the last call, by subject.
fn published(world: &mut World) -> Vec<(EntityId, EntityEvent<ReplicationData>)> {
    let deltas = world.resource_mut::<TileEvents>().take();
    deltas
        .tiles
        .into_iter()
        .flat_map(|delta| delta.events)
        .map(|event| (event.subject, event.event))
        .collect()
}

/// The ticks of `ticks` in a row, with what each published.
fn run_publishing(
    app: &mut bevy_app::App,
    ticks: u32,
) -> Vec<(EntityId, EntityEvent<ReplicationData>)> {
    let mut all = Vec::new();
    for _ in 0..ticks {
        run(app, 1);
        all.extend(published(app.world_mut()));
    }
    all
}

/// The same throw, merge and pickup as the throw test, seen from the network.
#[test]
fn ien_net_001_003_004_the_spawns_the_merged_stack_and_the_take_before_the_removal() {
    let mut app = app();
    let world = app.world_mut();
    let feet = DVec3::new(0.5, FLOOR, 0.5);
    let player = collector(world, DVec3::new(10.5, FLOOR, 0.5), Collect::Survival);
    launch(
        world,
        STONE,
        10,
        Launch {
            source: BirthSource::Entity(EntityKey(PLAYER)),
            form: Form::Hand {
                feet,
                eye_height: 1.62,
                velocity: HandVelocity::Look {
                    yaw: 0.0,
                    pitch: 90.0,
                },
            },
            values: ItemEntity::with_pickup_delay(Cause::Throw.pickup_delay()),
        },
    );
    launch(
        world,
        STONE,
        10,
        Launch {
            source: BirthSource::Block(BlockPos::new(0, 0, 0)),
            form: Form::Fixed(Start {
                position: DVec3::new(0.6, FLOOR, 0.5),
                velocity: DVec3::ZERO,
            }),
            values: ItemEntity::default(),
        },
    );

    let first = run_publishing(&mut app, 1);
    let spawns: Vec<i32> = first
        .iter()
        .filter_map(|(_, event)| match event {
            EntityEvent::Spawn(ReplicationData::Mob(MobReport::Spawn(spawn))) => {
                assert_eq!(spawn.kind, &EntityTypeData::ITEM);
                match &spawn.look {
                    SpawnLook::Item(stack) => Some(stack.count),
                    SpawnLook::Mob(_) => None,
                }
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        spawns,
        [10, 10],
        "IEN-NET-001: each spawn carries its stack"
    );

    let merge = run_publishing(&mut app, 79);
    let stacks: Vec<(EntityId, i32)> = merge
        .iter()
        .filter_map(|(subject, event)| match event {
            EntityEvent::Data(ReplicationData::Mob(MobReport::Stack(stack))) => {
                Some((*subject, stack.count))
            }
            _ => None,
        })
        .collect();
    let [(target, 20)] = stacks[..] else {
        panic!("IEN-NET-003: the target sends its new stack once: {stacks:?}");
    };
    let removed: Vec<EntityId> = merge
        .iter()
        .filter(|(_, event)| matches!(event, EntityEvent::Despawn))
        .map(|(subject, _)| *subject)
        .collect();
    assert_eq!(removed.len(), 1, "IEN-NET-004: the merged stack is removed");
    assert_ne!(removed[0], target);
    assert!(
        !merge.iter().any(|(_, event)| matches!(
            event,
            EntityEvent::Data(ReplicationData::Mob(MobReport::Signal(
                MobSignal::Taken { .. }
            )))
        )),
        "a merge takes nothing"
    );

    move_collector(app.world_mut(), player, feet);
    let pickup = run_publishing(&mut app, 1);
    let ends: Vec<&EntityEvent<ReplicationData>> = pickup
        .iter()
        .filter(|(subject, _)| *subject == target)
        .map(|(_, event)| event)
        .collect();
    assert_eq!(
        ends,
        [
            &EntityEvent::Data(ReplicationData::Mob(MobReport::Signal(MobSignal::Taken {
                collector: PLAYER_ID,
                amount: 20,
            }))),
            &EntityEvent::Despawn,
        ],
        "IEN-NET-004: the take goes before the removal"
    );
}

#[test]
fn ien_pck_005_a_partial_pickup_sends_the_take_of_the_part_before_the_new_stack() {
    let mut app = app();
    let world = app.world_mut();
    let feet = DVec3::new(0.5, FLOOR, 0.5);
    let player = collector(world, DVec3::new(10.5, FLOOR, 0.5), Collect::Survival);
    for index in 0..35 {
        fill_slot(world, player, index, DIRT, 64);
    }
    fill_slot(world, player, 35, STONE, 60);
    launch(
        world,
        STONE,
        10,
        Launch {
            source: BirthSource::Block(BlockPos::new(0, 0, 0)),
            form: Form::Fixed(Start {
                position: feet,
                velocity: DVec3::ZERO,
            }),
            values: ItemEntity::default(),
        },
    );
    run_publishing(&mut app, 1);

    move_collector(app.world_mut(), player, feet);
    let pickup = run_publishing(&mut app, 1);

    let data: Vec<String> = pickup
        .iter()
        .filter_map(|(_, event)| match event {
            EntityEvent::Data(ReplicationData::Mob(MobReport::Signal(MobSignal::Taken {
                collector,
                amount,
            }))) => Some(format!("take {amount} by {}", collector.0)),
            EntityEvent::Data(ReplicationData::Mob(MobReport::Stack(stack))) => {
                Some(format!("stack {}", stack.count))
            }
            EntityEvent::Despawn => Some("despawn".to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(
        data,
        [format!("take 4 by {}", PLAYER_ID.0), "stack 6".to_owned()],
        "the take of the 4 items that moved, then the stack of the rest, and no removal"
    );
}
