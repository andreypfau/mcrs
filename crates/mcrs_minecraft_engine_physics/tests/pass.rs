//! The entity pass gives the same rows and the same grid for any thread count, task split and
//! order of the task bins (PHY-INV-015, PHY-CMP-012, PHY-CMP-015), and the grid that the pass
//! writes and the freeze patches equals a grid built again from the rows.

use bevy_tasks::TaskPool;
use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_minecraft_engine_physics::EntityRows;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::EffectBuffer;
use mcrs_minecraft_physics::Keyed;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::WriterKey;

const TICKS: u64 = 60;

fn pool(threads: usize) -> TaskPool {
    TaskPoolBuilder::new().num_threads(threads).build()
}

fn scene() -> TestScene {
    TestScene::new(2, 400, 400, 7)
}

/// Impulses from two writers on the first rows of each tile, binned by two tasks and gathered in
/// the order `reversed` picks.
fn writes(rows: &mut EntityRows, tick: u64, reversed: bool) -> EntityWrites {
    let mut tasks = [EffectBuffer::default(), EffectBuffer::default()];
    for (tile, tile_rows) in rows.tiles_mut() {
        for (index, row) in tile_rows.rows_mut().take(3).enumerate() {
            for (task, buffer) in tasks.iter_mut().enumerate() {
                let writer = BirthKey::new(tick, SourceKind::Entity, task as u128, 0);
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
fn run(threads: usize, tiles_per_task: usize, reversed: bool) -> (String, TestScene) {
    let pool = pool(threads);
    let mut scene = scene();
    for tick in 0..TICKS {
        let mut writes = if tick % 10 == 3 {
            writes(&mut scene.rows, tick, reversed)
        } else {
            EntityWrites::default()
        };
        scene.step(&pool, tiles_per_task, &mut writes);
    }
    (format!("{:?}\n{:?}", scene.rows, scene.grid), scene)
}

#[test]
fn inv_015_the_pass_gives_the_same_state_for_each_thread_count_and_task_split() {
    let (expected, mut scene) = run(1, 1, false);
    let resting = scene
        .rows
        .tiles_mut()
        .flat_map(|(_, tile_rows)| tile_rows.rows_mut())
        .filter(|row| matches!(row.state.rest, Rest::Resting(_)))
        .count();
    assert!(resting > 100, "the items rest after the fall: {resting}");
    for (threads, tiles_per_task, reversed) in [(2, 3, true), (8, 1, false), (8, 100, true)] {
        let (actual, _) = run(threads, tiles_per_task, reversed);
        assert!(
            actual == expected,
            "{threads} threads, {tiles_per_task} tiles per task, reversed bins {reversed}"
        );
    }
}

#[test]
fn the_grid_that_the_pass_writes_and_the_freeze_patches_equals_a_rebuild() {
    let (_, mut scene) = run(4, 2, false);
    scene.rows.apply_writes(&mut EntityWrites::default());
    scene.rows.freeze(&pool(1), &mut scene.grid);

    let mut rebuilt = EntityRows::default();
    for (_, tile_rows) in scene.rows.tiles_mut() {
        for row in tile_rows.rows_mut() {
            rebuilt.spawn(row.id, *row.state, *row.motion);
        }
    }
    let mut grid = BodyGrid::default();
    rebuilt.freeze(&pool(1), &mut grid);
    for (tile, _) in scene.rows.tiles_mut() {
        assert_eq!(scene.grid.front(tile), grid.front(tile), "tile {tile:?}");
    }
    assert!(
        scene.rows.tiles_mut().count() > 4,
        "the rows spread over several tiles, so the patch moved rows between tiles"
    );
}
