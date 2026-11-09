//! The containers of the rows across tiles and tasks: the bins of the merge point, the swap of
//! the grid buffers at the freeze, and the tile of a living row as it crosses, dies and leaves.

use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::EffectBuffer;
use mcrs_minecraft_entity_rows::Executor;
use mcrs_minecraft_entity_rows::Keyed;
use mcrs_minecraft_entity_rows::Pass;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::TileBodies;
use mcrs_minecraft_entity_rows::TileQueues;
use mcrs_minecraft_entity_rows::WriterKey;
use mcrs_minecraft_entity_rows::YzxPos;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_voxel::BlockPos;

const TILE: TileKey = TileKey::new(0, 0, 0);

fn key(tick: u64) -> EntityKey {
    EntityKey(u128::from(tick))
}

fn write(target: u64, writer: WriterKey, sequence: u32) -> Keyed<u32> {
    Keyed {
        target: key(target),
        target_id: EntityId(target),
        writer,
        sequence,
        effect: sequence,
    }
}

fn applied(queues: &mut TileQueues<Keyed<u32>>) -> Vec<(u64, WriterKey, u32)> {
    queues
        .take_in_order(TILE)
        .into_iter()
        .map(|entry| (entry.target.0 as u64, entry.writer, entry.sequence))
        .collect()
}

#[test]
fn cmp_015_017_bins_apply_in_key_order_whatever_task_made_them() {
    let block_low = WriterKey::Block {
        pos: YzxPos::from(BlockPos::new(9, 0, 5)),
    };
    let block_high = WriterKey::Block {
        pos: YzxPos::from(BlockPos::new(0, 1, 0)),
    };
    let entity = WriterKey::Entity { key: key(3) };
    let command = WriterKey::Command { sequence: 7 };
    let entries = [
        write(2, block_high, 0),
        write(1, entity, 1),
        write(1, block_low, 0),
        write(1, entity, 0),
        write(1, block_high, 0),
        write(1, command, 0),
    ];

    let mut by_one_task = TileQueues::default();
    let mut one = EffectBuffer::default();
    for entry in entries {
        one.push(TILE, entry);
    }
    by_one_task.gather(&mut one);

    let mut by_two_tasks = TileQueues::default();
    let mut first = EffectBuffer::default();
    let mut second = EffectBuffer::default();
    for (index, entry) in entries.into_iter().rev().enumerate() {
        let buffer = if index % 2 == 0 {
            &mut first
        } else {
            &mut second
        };
        buffer.push(TILE, entry);
        buffer.push(TileKey::new(1, 0, 0), entry);
    }
    by_two_tasks.gather(&mut second);
    by_two_tasks.gather(&mut first);

    let expected = vec![
        (1, command, 0),
        (1, entity, 0),
        (1, entity, 1),
        (1, block_low, 0),
        (1, block_high, 0),
        (2, block_high, 0),
    ];
    assert_eq!(applied(&mut by_one_task), expected);
    assert_eq!(applied(&mut by_two_tasks), expected);
    assert_eq!(
        by_two_tasks.take(TileKey::new(1, 0, 0)).len(),
        6,
        "the bins of another tile stay in its queue"
    );
}

fn grid_row(x: f64) -> GridRow {
    GridRow {
        position: DVec3::new(x, 0.0, 0.0),
        half_width: 0.3,
        height: 1.8,
        key: key(0),
        id: EntityId(0),
        root_vehicle: EntityId(0),
        team: TeamSlot::default(),
        flags: GridFlags::empty(),
        push_targets: PushTargets::default(),
    }
}

#[test]
fn the_freeze_swaps_only_written_tiles_and_gives_back_the_old_front() {
    let bodies = |x: f64| TileBodies {
        rows: vec![grid_row(x)],
        starts: vec![0, 1],
    };
    let mut grid = BodyGrid::default();
    let quiet = TileKey::new(1, 0, 0);
    let mut back = bodies(1.0);
    grid.swap(TILE, &mut back);
    grid.swap(quiet, &mut bodies(9.0));
    assert_eq!(back, TileBodies::default(), "a new tile has no old front");

    let mut back = bodies(2.0);
    grid.swap(TILE, &mut back);
    assert_eq!(
        grid.front(TILE),
        &bodies(2.0),
        "the back buffer becomes the front"
    );
    assert_eq!(
        back,
        bodies(1.0),
        "the old front comes back for the next write"
    );
    assert_eq!(
        grid.front(quiet),
        &bodies(9.0),
        "a quiet tile keeps its front"
    );
}

/// Runs each task in turn on the calling thread.
struct Serial;

impl Executor for Serial {
    fn threads(&self) -> usize {
        1
    }

    fn run<T, R, F>(&self, tasks: Vec<T>, work: &F) -> Vec<R>
    where
        T: Send,
        R: Send + 'static,
        F: Fn(T) -> R + Sync,
    {
        tasks.into_iter().map(work).collect()
    }
}

fn walker(x: f64, velocity: f64) -> EntityState {
    let motion = MotionRow::zombie();
    let mut state = EntityState::new(
        key(1),
        Body {
            position: DVec3::new(x, 64.0, 0.5),
            velocity: DVec3::new(velocity, 0.0, 0.0),
            ..Body::default()
        },
        Some(Living::of(&motion)),
    );
    state.no_gravity = true;
    state
}

#[test]
fn the_store_follows_the_tile_of_a_living_row_that_crosses_dies_and_leaves() {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table);
    let world = TestWorld::new();
    let pass = Pass {
        world: &world,
        tables: &tables,
        tick: 0,
        push: PushRules::default(),
        tiles_per_task: 1,
    };
    let mut grid = BodyGrid::default();
    let mut rows: RowStore<()> = RowStore::default();
    let (mover, staying) = (EntityId(1), EntityId(2));
    let start = TileKey::new(0, 0, 0);
    let next = TileKey::new(1, 0, 0);
    rows.spawn(mover, walker(31.9, 0.5), MotionRow::zombie());
    rows.spawn(staying, walker(4.5, 0.0), MotionRow::zombie());
    assert_eq!(rows.tile_of(mover), Some(start));
    assert_eq!(
        rows.changes().spawned(),
        &[
            Placed {
                tile: start,
                id: mover
            },
            Placed {
                tile: start,
                id: staying
            }
        ]
    );

    rows.freeze(&Serial, &mut grid);
    rows.tick(&pass, &Serial, &grid);
    assert!(
        rows.changes().spawned().is_empty(),
        "the pass ends the spawns"
    );
    assert_eq!(
        rows.tile_of(mover),
        Some(start),
        "the row moves at the tile moves"
    );
    rows.move_rows();
    assert_eq!(
        rows.tile_of(mover),
        Some(next),
        "the tile moves follow the row"
    );
    assert_eq!(rows.changes().crossed().len(), 1);
    assert_eq!(
        rows.changes()
            .crossed()
            .first()
            .map(|crossing| (crossing.id, crossing.from, crossing.to)),
        Some((mover, start, next))
    );

    let dying = Placed {
        tile: next,
        id: mover,
    };
    if let Some(living) = rows
        .state_mut(dying)
        .and_then(|state| state.living.as_mut())
    {
        living.death_time = Some(19);
    }
    rows.freeze(&Serial, &mut grid);
    rows.tick(&pass, &Serial, &grid);
    assert_eq!(
        rows.tile_of(mover),
        None,
        "the end of the death removes the row"
    );
    assert_eq!(rows.changes().removed(), &[dying]);

    rows.move_rows();
    assert!(rows.changes().crossed().is_empty());
    let left = rows.despawn(start, staying).map(|row| row.id);
    assert_eq!(left, Some(staying));
    assert_eq!(rows.tile_of(staying), None, "a despawn removes the row");
    assert_eq!(
        rows.changes().removed(),
        &[
            dying,
            Placed {
                tile: start,
                id: staying
            }
        ]
    );
}
