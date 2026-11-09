//! The containers of the rows across tiles and tasks: the bins of the merge point, the swap of
//! the grid buffers at the freeze and the reserved ids of a tile.

use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::EffectBuffer;
use mcrs_minecraft_entity_rows::Keyed;
use mcrs_minecraft_entity_rows::TileBodies;
use mcrs_minecraft_entity_rows::TileQueues;
use mcrs_minecraft_entity_rows::TileRows;
use mcrs_minecraft_entity_rows::WriterKey;
use mcrs_minecraft_entity_rows::YzxPos;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::TeamSlot;
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

fn state(x: f64) -> EntityState {
    EntityState::new(
        key(0),
        Body {
            position: DVec3::new(x, 0.0, 0.0),
            ..Body::default()
        },
        None,
    )
}

#[test]
fn rows_spawn_with_reserved_ids_and_despawn_by_id() {
    let mut rows: TileRows<()> = TileRows::default();
    assert_eq!(
        rows.spawn_row(state(0.0), MotionRow::item()),
        None,
        "no id reserved"
    );
    rows.reserve_ids([EntityId(10), EntityId(12)]);
    let first = rows
        .spawn_row(state(1.0), MotionRow::item())
        .expect("a reserved id");
    let second = rows
        .spawn_row(state(2.0), MotionRow::item())
        .expect("a reserved id");
    assert_ne!(first, second);
    assert_eq!(
        rows.spawn_row(state(3.0), MotionRow::item()),
        None,
        "both ids are used"
    );

    assert_eq!(
        rows.despawn_row(first).map(|row| row.state),
        Some(state(1.0))
    );
    assert_eq!(rows.despawn_row(first), None, "the row is gone");
    assert_eq!(rows.ids(), &[second]);
}
