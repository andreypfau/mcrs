//! The entity pass gives the same rows and the same grid for any thread count, task split and
//! order of the task bins (PHY-INV-015, PHY-CMP-012, PHY-CMP-015), and the grid that the pass
//! writes and the freeze patches equals a grid built again from the rows.

use bevy_tasks::TaskPool;
use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::EffectBuffer;
use mcrs_minecraft_entity_rows::Keyed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::WriterKey;
use mcrs_minecraft_physics::Rest;

const TICKS: u64 = 60;

fn pool(threads: usize) -> TaskPool {
    TaskPoolBuilder::new().num_threads(threads).build()
}

fn dense_scene() -> TestScene {
    TestScene::<()>::new(2, 400, 400, 7)
}

/// Impulses from two writers on the first rows of each tile, binned by two tasks and gathered in
/// the order `reversed` picks.
fn writes(rows: &RowStore<()>, tick: u64, reversed: bool) -> EntityWrites {
    let mut tasks = [EffectBuffer::default(), EffectBuffer::default()];
    for (tile, tile_rows) in rows.tiles() {
        for (index, row) in tile_rows.rows().take(3).enumerate() {
            for (task, buffer) in tasks.iter_mut().enumerate() {
                let writer = EntityKey(u128::from(tick) << 64 | task as u128);
                buffer.push(
                    tile,
                    Keyed {
                        target: row.state.key,
                        target_id: row.id,
                        writer: WriterKey::Entity { key: writer },
                        sequence: index as u32,
                        effect: DVec3::new(0.1 / 3.0, 0.0, 0.07 * (task as f64 + 1.0)),
                    },
                );
            }
        }
    }
    if reversed {
        tasks.reverse();
    }
    let mut writes = EntityWrites::default();
    for buffer in &mut tasks {
        writes.0.gather(buffer);
    }
    writes
}

/// The rows and the grid after the run, as text that tells every bit of each value.
fn run(
    mut scene: TestScene,
    threads: usize,
    tiles_per_task: usize,
    reversed: bool,
) -> (String, TestScene) {
    let pool = pool(threads);
    for tick in 0..TICKS {
        let mut writes = if tick % 10 == 3 {
            writes(&scene.rows, tick, reversed)
        } else {
            EntityWrites::default()
        };
        scene.step(&pool, tiles_per_task, &mut writes);
    }
    (format!("{:?}\n{:?}", scene.rows, scene.grid), scene)
}

#[test]
fn inv_015_the_pass_gives_the_same_state_for_each_thread_count_and_task_split() {
    let (expected, scene) = run(dense_scene(), 1, 1, false);
    let resting = scene
        .rows
        .tiles()
        .flat_map(|(_, tile_rows)| tile_rows.rows())
        .filter(|row| matches!(row.state.rest, Rest::Resting(_)))
        .count();
    assert!(resting > 100, "the items rest after the fall: {resting}");
    for (threads, tiles_per_task, reversed) in [(2, 3, true), (8, 1, false), (8, 100, true)] {
        let (actual, _) = run(dense_scene(), threads, tiles_per_task, reversed);
        assert!(
            actual == expected,
            "{threads} threads, {tiles_per_task} tiles per task, reversed bins {reversed}"
        );
    }
}

#[test]
fn the_grid_that_the_pass_writes_and_the_freeze_patches_equals_a_rebuild() {
    // The sparse scene has tiles in which every body walks, and tiles with no body that moves.
    for start in [dense_scene(), TestScene::<()>::new(2, 12, 0, 3)] {
        let (_, mut scene) = run(start, 4, 2, false);
        scene.rows.apply_writes(&mut EntityWrites::default().0);
        scene.rows.freeze(&pool(1), &mut scene.grid);

        let mut rebuilt: RowStore<()> = RowStore::default();
        for (_, tile_rows) in scene.rows.tiles() {
            for row in tile_rows.rows() {
                rebuilt.spawn(row.id, *row.state, *row.motion);
            }
        }
        let mut grid = BodyGrid::default();
        rebuilt.freeze(&pool(1), &mut grid);
        for (tile, _) in scene.rows.tiles() {
            assert_eq!(
                scene.grid.front(tile).rows,
                grid.front(tile).rows,
                "tile {tile:?}"
            );
        }
        assert!(
            scene.rows.tiles().count() > 4,
            "the rows spread over several tiles, so the patch moved rows between tiles"
        );
    }
}

#[test]
fn cmp_015_a_write_adds_its_impulse_to_the_velocity_of_its_target_and_wakes_it() {
    let (_, mut scene) = run(dense_scene(), 1, 1, false);
    let mut resting = Vec::new();
    for (tile, tile_rows) in scene.rows.tiles() {
        for row in tile_rows.rows() {
            if matches!(row.state.rest, Rest::Resting(_)) {
                resting.push((tile, row.id, row.state.key, row.state.body.velocity));
            }
        }
    }
    let [
        (tile, target, key, velocity),
        (_, other, _, other_velocity),
        ..,
    ] = resting[..]
    else {
        panic!("two resting rows: {}", resting.len());
    };
    let effect = DVec3::new(0.25, 0.5, -0.125);
    let mut buffer = EffectBuffer::default();
    buffer.push(
        tile,
        Keyed {
            target: key,
            target_id: target,
            writer: WriterKey::Entity { key: EntityKey(0) },
            sequence: 0,
            effect,
        },
    );
    let mut writes = EntityWrites::default();
    writes.0.gather(&mut buffer);
    scene.rows.apply_writes(&mut writes.0);
    let after: Vec<_> = scene
        .rows
        .tiles()
        .flat_map(|(_, tile_rows)| tile_rows.rows())
        .filter(|row| row.id == target || row.id == other)
        .map(|row| (row.id, row.state.body.velocity, row.state.rest))
        .collect();
    for (id, velocity_after, rest) in after {
        if id == target {
            assert_eq!(
                velocity_after,
                velocity + effect,
                "the target adds the impulse"
            );
            assert_eq!(rest, Rest::Awake, "the write wakes the target");
        } else {
            assert_eq!(
                velocity_after, other_velocity,
                "another row keeps its velocity"
            );
        }
    }
}
