use bevy_ecs::world::World;
use glam::DVec3;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobState;
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

fn published(world: &mut World) -> Vec<(EntityId, EntityEvent<ReplicationData>)> {
    let deltas = world.resource_mut::<TileEvents>().take();
    deltas
        .tiles
        .into_iter()
        .flat_map(|delta| delta.events)
        .map(|event| (event.subject, event.event))
        .collect()
}

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
            source: BirthSource::Entity(EntityUuid::from_u128(PLAYER)),
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
                    _ => None,
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

#[test]
fn item_take_and_removal_across_tile_edge_go_to_spawn_tile() {
    let mut app = app();
    let world = app.world_mut();
    collector(world, DVec3::new(1.0, FLOOR, 0.5), Collect::Survival);
    launch(
        world,
        STONE,
        1,
        Launch {
            source: BirthSource::Block(BlockPos::new(0, 0, 0)),
            form: Form::Fixed(Start {
                position: DVec3::new(-1.0, FLOOR + 1.0, 0.5),
                velocity: DVec3::new(0.6, 0.0, 0.0),
            }),
            values: ItemEntity::with_pickup_delay(2),
        },
    );
    let spawn_tile = TileKey::at(DVec3::new(-1.0, FLOOR, 0.5));
    let spawned = published_by_tile(&mut app);
    assert!(
        spawned
            .iter()
            .any(|(tile, _, event)| *tile == spawn_tile && matches!(event, EntityEvent::Spawn(_))),
        "the item spawns west of the edge: {spawned:?}"
    );

    let crossing = published_by_tile(&mut app);

    let kinds: Vec<(TileKey, &str)> = crossing
        .iter()
        .filter(|(_, subject, _)| *subject != PLAYER_ID)
        .map(|(tile, _, event)| {
            let kind = match event {
                EntityEvent::Data(ReplicationData::Mob(MobReport::Signal(MobSignal::Taken {
                    ..
                }))) => "take",
                EntityEvent::Despawn => "despawn",
                EntityEvent::Leave { .. } => "leave",
                EntityEvent::Enter { .. } => "enter",
                EntityEvent::Move(_) => "move",
                _ => "other",
            };
            (*tile, kind)
        })
        .collect();
    assert_eq!(
        kinds,
        [(spawn_tile, "take"), (spawn_tile, "despawn")],
        "the viewers of the spawn tile know the item"
    );
}

#[test]
fn an_item_at_rest_whose_velocity_changes_at_the_same_position_sends_a_move() {
    let mut app = app();
    let feet = DVec3::new(0.5, FLOOR, 0.5);
    let item = launch(
        app.world_mut(),
        STONE,
        1,
        Launch {
            source: BirthSource::Block(BlockPos::new(0, 0, 0)),
            form: Form::Fixed(Start {
                position: feet,
                velocity: DVec3::ZERO,
            }),
            values: ItemEntity::default(),
        },
    );
    run_publishing(&mut app, 100);
    let rest = app.world().get::<MotionState>(item).expect("a motion").0;
    assert!(matches!(rest.rest, Rest::Resting(_)), "the item rests");
    let rest = rest.body;
    app.world_mut()
        .get_mut::<MotionState>(item)
        .expect("a motion")
        .0
        .body
        .velocity = DVec3::new(0.1, 0.0, 0.0);

    let woken = run_publishing(&mut app, 1);

    let moves: Vec<MobState> = woken
        .iter()
        .filter_map(|(_, event)| match event {
            EntityEvent::Move(ReplicationData::Mob(MobReport::Move(state))) => Some(*state),
            _ => None,
        })
        .collect();
    let [moved] = moves[..] else {
        panic!("one move: {woken:?}");
    };
    assert!(
        moved.position.x > rest.position.x && moved.velocity.x > 0.0,
        "the move carries the written velocity: {moved:?}"
    );

    run_publishing(&mut app, 100);
    let rest = app.world().get::<MotionState>(item).expect("a motion").0;
    assert!(
        matches!(rest.rest, Rest::Resting(_)),
        "the item rests again"
    );
    let silent = run_publishing(&mut app, 40);
    assert!(
        silent.is_empty(),
        "an item at rest publishes nothing: {silent:?}"
    );
}

fn published_by_tile(
    app: &mut bevy_app::App,
) -> Vec<(TileKey, EntityId, EntityEvent<ReplicationData>)> {
    run(app, 1);
    let deltas = app.world_mut().resource_mut::<TileEvents>().take();
    deltas
        .tiles
        .into_iter()
        .flat_map(|delta| {
            let tile = delta.tile;
            delta
                .events
                .into_iter()
                .map(move |event| (tile, event.subject, event.event))
        })
        .collect()
}
