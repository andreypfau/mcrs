use std::collections::BTreeMap;

use glam::DVec3;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_entity_motion::MotionHold;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::EntityWrite;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::TileBodies;
use mcrs_minecraft_physics::grid_row;

const TICKS: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpawnOrder {
    Forward,
    Reverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WriteOrder {
    Forward,
    Reverse,
}

fn dense_scene(order: SpawnOrder) -> TestScene {
    let mut scene = TestScene::new(2, 400, 400, 7);
    if order == SpawnOrder::Reverse {
        let world = scene.app.world_mut();
        let mut query = world.query::<(&MotionState, &MotionKind, &MotionId)>();
        let mut bodies: Vec<_> = query
            .iter(world)
            .map(|(state, kind, id)| (state.0, kind.0, id.0))
            .collect();
        bodies.sort_by_key(|(state, _, _)| std::cmp::Reverse(state.key));
        let entities: Vec<_> = scene
            .states()
            .into_iter()
            .map(|(entity, _)| entity)
            .collect();
        for entity in entities {
            scene.app.world_mut().despawn(entity);
        }
        for (state, kind, id) in bodies {
            scene.spawn(id, state, kind);
        }
    }
    scene
}

fn write(scene: &mut TestScene, tick: u64, order: WriteOrder) {
    let mut tiles: BTreeMap<TileKey, Vec<EntityUuid>> = BTreeMap::new();
    for (_, state) in scene.states() {
        let keys = tiles.entry(TileKey::at(state.body.position)).or_default();
        if keys.len() < 3 {
            keys.push(state.key);
        }
    }
    let mut tasks = [Vec::new(), Vec::new()];
    for keys in tiles.values() {
        for (index, &target) in keys.iter().enumerate() {
            for (task, writes) in tasks.iter_mut().enumerate() {
                let writer = EntityUuid::from_u128(u128::from(tick) << 64 | task as u128);
                writes.push(EntityWrite {
                    target,
                    writer: WriterKey::Entity { key: writer },
                    sequence: index as u32,
                    effect: DVec3::new(0.1 / 3.0, 0.0, 0.07 * (task as f64 + 1.0)),
                });
            }
        }
    }
    if order == WriteOrder::Reverse {
        tasks.reverse();
    }
    let mut writes = scene.app.world_mut().resource_mut::<EntityWrites>();
    for write in tasks.into_iter().flatten() {
        writes.push(write);
    }
}

fn run(mut scene: TestScene, order: WriteOrder) -> (String, TestScene) {
    for tick in 0..TICKS {
        if tick % 10 == 3 {
            write(&mut scene, tick, order);
        }
        scene.step();
    }
    let states: Vec<EntityState> = scene.states().into_iter().map(|(_, state)| state).collect();
    let grid = scene.app.world().resource::<BodyGrid>();
    let mut tiles: Vec<TileKey> = (states.iter())
        .map(|state| TileKey::at(state.body.position))
        .collect();
    tiles.sort_unstable();
    tiles.dedup();
    let rows: Vec<&[GridRow]> = tiles.iter().map(|&tile| grid.tile(tile).rows()).collect();
    (format!("{states:?}\n{rows:?}"), scene)
}

#[test]
fn inv_015_the_pass_gives_the_same_state_for_each_spawn_order_and_write_order() {
    let (expected, mut scene) = run(dense_scene(SpawnOrder::Forward), WriteOrder::Forward);
    let resting = (scene.states().iter())
        .filter(|(_, state)| matches!(state.rest, Rest::Resting(_)))
        .count();
    assert!(resting > 100, "the items rest after the fall: {resting}");
    for (spawns, writes) in [
        (SpawnOrder::Forward, WriteOrder::Reverse),
        (SpawnOrder::Reverse, WriteOrder::Forward),
        (SpawnOrder::Reverse, WriteOrder::Reverse),
    ] {
        let (actual, _) = run(dense_scene(spawns), writes);
        assert!(actual == expected, "{spawns:?} spawns, {writes:?} writes");
    }
}

#[test]
fn the_grid_that_the_pass_keeps_equals_a_rebuild() {
    for start in [
        dense_scene(SpawnOrder::Forward),
        TestScene::new(2, 12, 0, 3),
    ] {
        let (_, mut scene) = run(start, WriteOrder::Forward);
        let world = scene.app.world_mut();
        let mut query = world.query::<(&MotionState, &MotionKind, &MotionId, Option<&Vitals>)>();
        let rows: Vec<GridRow> = query
            .iter(world)
            .filter_map(|(state, kind, id, vitals)| {
                let flags = if vitals.is_none_or(Vitals::is_alive) {
                    GridFlags::PUSHABLE
                } else {
                    GridFlags::empty()
                };
                grid_row(&state.0, &kind.0, id.0, flags)
            })
            .collect();
        scene.step();

        let mut tiles: BTreeMap<TileKey, Vec<GridRow>> = BTreeMap::new();
        for row in rows {
            tiles
                .entry(TileKey::at(row.position))
                .or_default()
                .push(row);
        }
        let grid = scene.app.world().resource::<BodyGrid>();
        for (&tile, inside) in &tiles {
            let mut expected = TileBodies::default();
            expected.fill(tile, inside);
            assert_eq!(grid.tile(tile).rows(), expected.rows(), "tile {tile:?}");
        }
        for &tile in tiles.keys() {
            for (dx, dz) in [
                (-1, -1),
                (-1, 0),
                (-1, 1),
                (0, -1),
                (0, 1),
                (1, -1),
                (1, 0),
                (1, 1),
            ] {
                let neighbour = TileKey::new(tile.x + dx, tile.y, tile.z + dz);
                if !tiles.contains_key(&neighbour) {
                    assert!(
                        grid.tile(neighbour).rows().is_empty(),
                        "tile {neighbour:?} holds no body, so a body that left it left no row"
                    );
                }
            }
        }
        assert!(
            tiles.len() > 4,
            "the bodies spread over several tiles, so the grid moved rows between tiles"
        );
    }
}

#[test]
fn cmp_015_a_write_adds_its_impulse_to_the_velocity_of_its_target_and_wakes_it() {
    let (_, mut scene) = run(dense_scene(SpawnOrder::Forward), WriteOrder::Forward);
    let resting: Vec<_> = (scene.states().into_iter())
        .filter(|(_, state)| matches!(state.rest, Rest::Resting(_)))
        .collect();
    let [(target, before), (other, other_before), ..] = resting[..] else {
        panic!("two resting bodies: {}", resting.len());
    };
    for entity in [target, other] {
        scene.app.world_mut().entity_mut(entity).insert(MotionHold);
    }
    let effect = DVec3::new(0.25, 0.5, -0.125);
    scene
        .app
        .world_mut()
        .resource_mut::<EntityWrites>()
        .push(EntityWrite {
            target: before.key,
            writer: WriterKey::Entity {
                key: EntityUuid::from_u128(0),
            },
            sequence: 0,
            effect,
        });
    scene.step();
    let after = scene.state(target).expect("the target stays");
    assert_eq!(
        after.body.velocity,
        before.body.velocity + effect,
        "the target adds the impulse"
    );
    assert_eq!(after.rest, Rest::Awake, "the write wakes the target");
    assert_eq!(
        scene
            .state(other)
            .expect("the other body stays")
            .body
            .velocity,
        other_before.body.velocity,
        "another body keeps its velocity"
    );
}
