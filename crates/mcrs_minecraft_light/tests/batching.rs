use bevy_ecs::prelude::Entity;
mod common;

use std::sync::Arc;

use common::*;

fn drain_one(queue: &mut LightQueue, budget_cells: u64, avoid: &[BlockBox]) -> Vec<Influence> {
    queue
        .drain_batches(budget_cells, avoid, 1)
        .pop()
        .unwrap_or_default()
}
use mcrs_minecraft_light::field::FieldLayout;
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_math::{BlockPos, ChunkPos, ColumnPos};
use mcrs_voxel_storage::VoxelId;

fn column(x: i32, z: i32) -> ColumnPos {
    ColumnPos { x, z }
}

fn influence_at(x: i32, y: i32, z: i32) -> Influence {
    Influence::around(BlockPos::new(x, y, z))
}

#[test]
fn the_most_urgent_column_is_handed_out_first() {
    let mut queue = LightQueue::default();
    queue.push_with_priority(column(5, 5), influence_at(80, 8, 80), 30);
    queue.push_with_priority(column(1, 1), influence_at(16, 8, 16), 10);
    queue.push_with_priority(column(3, 3), influence_at(48, 8, 48), 20);

    assert_eq!(queue.len(), 3);
    // One influence per column, so a budget of one takes them one at a time.
    let first = drain_one(&mut queue, 1, &[]);
    assert_eq!(first.len(), 1);
    assert_eq!(
        first[0].core.min,
        BlockPos::new(16, 8, 16),
        "priority 10 first"
    );
    assert_eq!(
        drain_one(&mut queue, 1, &[])[0].core.min,
        BlockPos::new(48, 8, 48)
    );
    assert_eq!(
        drain_one(&mut queue, 1, &[])[0].core.min,
        BlockPos::new(80, 8, 80)
    );
    assert!(queue.is_empty());
}

#[test]
fn a_column_keeps_the_most_urgent_priority_it_was_given() {
    let mut queue = LightQueue::default();
    queue.push_with_priority(column(2, 2), influence_at(32, 8, 32), 50);
    queue.push_with_priority(column(2, 2), influence_at(33, 8, 32), 5);
    assert_eq!(queue.priority_of(column(2, 2)), Some(5));
    assert_eq!(
        drain_one(&mut queue, u64::MAX, &[]).len(),
        2,
        "both pieces of work are still there"
    );
}

#[test]
fn a_column_can_be_pulled_forward_after_it_was_queued() {
    let mut queue = LightQueue::default();
    queue.push_with_priority(column(9, 9), influence_at(144, 8, 144), 100);
    queue.push_with_priority(column(1, 1), influence_at(16, 8, 16), 10);

    // The player walked towards the far column, as vanilla re-sorts chunk tasks
    // when a ticket level changes.
    queue.set_priority(column(9, 9), 1);
    assert_eq!(
        drain_one(&mut queue, 1, &[])[0].core.min,
        BlockPos::new(144, 8, 144),
        "the re-prioritised column now goes first"
    );
}

#[test]
fn a_column_bigger_than_the_budget_still_makes_progress() {
    let mut queue = LightQueue::default();
    for i in 0..10 {
        queue.push(column(0, 0), influence_at(i, 8, 0));
    }
    let batch = drain_one(&mut queue, 1, &[]);
    assert_eq!(batch.len(), 10, "columns are taken whole, never split");
    assert!(queue.is_empty());
}

/// The queue measures a batch the way `prepare_batch` will allocate it, so the
/// two have to agree on what a box costs.
#[test]
fn the_queue_and_the_field_agree_on_what_an_area_costs() {
    for area in [
        BlockBox {
            min: BlockPos::new(0, 0, 0),
            max: BlockPos::new(15, 15, 15),
        },
        BlockBox {
            min: BlockPos::new(-17, -1, 3),
            max: BlockPos::new(40, 62, 16),
        },
    ] {
        let layout = FieldLayout::covering(area);
        assert_eq!(layout.block_bounds(), area.section_aligned(), "{area:?}");
        assert_eq!(
            layout.cell_count() as u64,
            area.section_aligned().cells(),
            "{area:?}"
        );
    }
}

/// The budget is a field volume, so a column far from the ones already taken is
/// what crosses it — that is what keeps a batch spatially coherent.
#[test]
fn a_column_that_would_blow_up_the_field_waits_for_the_next_batch() {
    let mut queue = LightQueue::default();
    queue.push_with_priority(column(0, 0), influence_at(8, 8, 8), 0);
    queue.push_with_priority(column(1, 0), influence_at(24, 8, 8), 1);
    queue.push_with_priority(column(60, 0), influence_at(968, 8, 8), 2);

    // Wide enough for two neighbouring columns, nowhere near wide enough to
    // reach the far one.
    let batch = drain_one(&mut queue, 6 * 3 * 3 * 4096, &[]);
    assert_eq!(batch.len(), 2, "the two neighbours went together");
    assert_eq!(queue.len(), 1, "the far column waits");
    assert_eq!(
        drain_one(&mut queue, 1, &[])[0].core.min,
        BlockPos::new(968, 8, 8)
    );
}

/// A batch that overlapped work already in flight would publish whole sections
/// of its own field over the other job's answer, or have its own overwritten.
#[test]
fn a_column_under_work_in_flight_is_left_for_the_next_tick() {
    let mut queue = LightQueue::default();
    queue.push_with_priority(column(0, 0), influence_at(8, 8, 8), 0);
    queue.push_with_priority(column(60, 0), influence_at(968, 8, 8), 1);

    let in_flight = Influence::around(BlockPos::new(8, 8, 8))
        .bounds()
        .expand(1)
        .section_aligned();
    let batch = drain_one(&mut queue, u64::MAX, &[in_flight]);
    assert_eq!(batch.len(), 1);
    assert_eq!(
        batch[0].core.min,
        BlockPos::new(968, 8, 8),
        "only the column clear of the job in flight was taken"
    );
    assert_eq!(queue.len(), 1);
}

/// The whole of the concurrency argument: two jobs whose fields are disjoint
/// change no cell the other reads, so the order they publish in cannot matter —
/// not even when the older job was prepared before the newer job's edit existed.
#[test]
fn disjoint_jobs_publish_in_any_order() {
    let near = BlockPos::new(8, 8, 8);
    let far = BlockPos::new(72, 8, 72);

    let mut in_one_pass = TestWorld::new(5, 3, 5);
    in_one_pass.set_many([(near, GLOWSTONE), (far, TORCH)]);

    let mut concurrent = TestWorld::new(5, 3, 5);
    let mut queue = LightQueue::default();
    for (col, influence) in concurrent.world.apply_edits([Edit::SetBlock {
        pos: near,
        block: GLOWSTONE,
    }]) {
        queue.push(col, influence);
    }
    let older = concurrent
        .world
        .prepare_batch(drain_one(&mut queue, 1, &[]))
        .expect("there was work");

    // The second edit lands while the first job is already in flight, so the
    // older job's block snapshot does not contain it.
    for (col, influence) in concurrent.world.apply_edits([Edit::SetBlock {
        pos: far,
        block: TORCH,
    }]) {
        queue.push(col, influence);
    }
    let batch = drain_one(&mut queue, 1, &[older.area()]);
    assert!(
        !batch.is_empty(),
        "the far column is clear of the older job"
    );
    let newer = concurrent
        .world
        .prepare_batch(batch)
        .expect("there was work");

    let (older, newer) = (older.run(), newer.run());
    concurrent.world.apply(newer);
    concurrent.world.apply(older);

    for y in 0..48 {
        for z in 0..80 {
            for x in 0..80 {
                let pos = BlockPos::new(x, y, z);
                assert_eq!(
                    in_one_pass.block_light(pos),
                    concurrent.world.light_at(pos, Layer::Block).get(),
                    "block light at {pos:?}"
                );
                assert_eq!(
                    in_one_pass.sky_light(pos),
                    concurrent.world.light_at(pos, Layer::Sky).get(),
                    "sky light at {pos:?}"
                );
            }
        }
    }
}

/// The point of the whole exercise: work spread over several passes has to end
/// up where one big pass would have.
#[test]
fn batching_reaches_the_same_answer_as_one_pass() {
    let edits: Vec<(BlockPos, VoxelId)> = vec![
        (BlockPos::new(8, 30, 8), GLOWSTONE),
        (BlockPos::new(40, 30, 8), TORCH),
        (BlockPos::new(8, 30, 40), TORCH),
        (BlockPos::new(40, 30, 40), GLOWSTONE),
        (BlockPos::new(24, 40, 24), STONE),
        (BlockPos::new(24, 39, 24), STONE),
        (BlockPos::new(20, 20, 20), WATER),
        (BlockPos::new(36, 22, 12), BOTTOM_SLAB),
    ];

    let mut in_one_pass = TestWorld::new(3, 3, 3);
    in_one_pass.set_many(edits.clone());

    let mut in_batches = TestWorld::new(3, 3, 3);
    let mut queue = LightQueue::default();
    for (pos, block) in &edits {
        for (col, influence) in in_batches.world.apply_edits([Edit::SetBlock {
            pos: *pos,
            block: *block,
        }]) {
            queue.push(col, influence);
        }
    }
    // One influence at a time: the smallest batch there is.
    let mut passes = 0;
    while !queue.is_empty() {
        let batch = drain_one(&mut queue, 1, &[]);
        if let Some(update) = in_batches.world.prepare_batch(batch).map(|job| job.run()) {
            in_batches.world.apply(update);
        }
        passes += 1;
        assert!(passes < 100, "the queue never drained");
    }
    // Fewer passes than edits: several of them share a chunk column, and
    // columns are handed out whole.
    assert!(
        passes > 1,
        "the work really was split across passes, got {passes}"
    );

    for y in 0..48 {
        for z in 0..48 {
            for x in 0..48 {
                let pos = BlockPos::new(x, y, z);
                assert_eq!(
                    in_one_pass.block_light(pos),
                    in_batches.world.light_at(pos, Layer::Block).get(),
                    "block light at {pos:?} after {passes} passes"
                );
                assert_eq!(
                    in_one_pass.sky_light(pos),
                    in_batches.world.light_at(pos, Layer::Sky).get(),
                    "sky light at {pos:?} after {passes} passes"
                );
            }
        }
    }
    in_batches.check_against_reference();
}

/// Holding lighting work back must not hold the block change back with it.
#[test]
fn an_edit_takes_effect_before_its_lighting_does() {
    let mut world = TestWorld::new(2, 2, 2);
    let pos = BlockPos::new(8, 8, 8);

    let work = world.world.apply_edits([Edit::SetBlock {
        pos,
        block: GLOWSTONE,
    }]);
    assert!(!work.is_empty(), "the edit created lighting work");

    assert_eq!(
        world.world.block_at(pos),
        GLOWSTONE,
        "the block is already there, only its light is pending"
    );
    assert_eq!(
        world.block_light(pos),
        0,
        "and the light has not been computed yet"
    );

    let update = world
        .world
        .prepare_batch(work.into_iter().map(|(_, influence)| influence))
        .map(|job| job.run())
        .expect("there was work");
    world.world.apply(update);
    assert_eq!(world.block_light(pos), 15);
}

#[test]
fn loading_a_stack_of_sections_produces_work_for_each() {
    let registry = registry();
    let mut world = LightWorld::new(registry, LightBounds::new(0, 3));
    let loads: Vec<Edit> = (0..4)
        .map(|y| Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(0, y, 0),
            blocks: Arc::new(filled(AIR)),
        })
        .collect();

    let work = world.apply_edits(loads);
    // One per section, and one more for the run of column that became a sky
    // source when the stack arrived.
    assert_eq!(work.len(), 5);
    assert!(
        work.iter().all(|(col, _)| *col == column(0, 0)),
        "all of it belongs to one chunk column"
    );
    let bounds = LightBounds::new(0, 3);
    assert!(
        work.iter().any(|(_, influence)| {
            influence.core.min.y == bounds.min_light_y()
                && influence.core.max.y == bounds.max_light_y()
        }),
        "the sky floor run spans the column the load lit"
    );
}

#[test]
fn loading_and_editing_a_column_in_one_batch_stays_inside_the_world() {
    let mut world = LightWorld::new(registry(), LightBounds::new(0, 3));
    let pos = BlockPos::new(1, 1, 1);

    let stats = world.update_now([
        Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(0, 0, 0),
            blocks: Arc::new(filled(AIR)),
        },
        Edit::SetBlock { pos, block: TORCH },
    ]);

    assert!(
        stats.area_cells < 1_000_000,
        "the working area stayed bounded, not stretched to the sky-floor sentinel: {}",
        stats.area_cells
    );
    assert_eq!(world.light_at(pos, Layer::Block).get(), 14);
}

/// Batches filled in one pass are handed out together, so they have to stay
/// disjoint against each other and not only against the work already running.
#[test]
fn batches_filled_in_one_pass_do_not_overlap() {
    let mut queue = LightQueue::default();
    for (index, x) in [8, 968, 1928, 2888].into_iter().enumerate() {
        queue.push_with_priority(column(x / 16, 0), influence_at(x, 8, 8), index as Priority);
    }

    let batches = queue.drain_batches(1, &[], 3);
    assert_eq!(batches.len(), 3, "one column each, budget of one cell");
    assert_eq!(queue.len(), 1, "the fourth column waits");

    let fields: Vec<_> = batches
        .iter()
        .map(|batch| batch[0].bounds().expand(1).section_aligned())
        .collect();
    for (i, a) in fields.iter().enumerate() {
        for b in &fields[i + 1..] {
            assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
        }
    }
    assert_eq!(
        batches[0][0].core.min,
        BlockPos::new(8, 8, 8),
        "the most urgent column opens the first batch"
    );
}
