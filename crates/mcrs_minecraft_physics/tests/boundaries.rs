use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BlockOverlay;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::ColliderEnds;
use mcrs_minecraft_physics::EffectBuffer;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Keyed;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::QueryEntity;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::TileQueues;
use mcrs_minecraft_physics::TileRows;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WriterKey;
use mcrs_minecraft_physics::YzxPos;
use mcrs_minecraft_physics::intersects;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

const TILE: TileKey = TileKey::new(0, 0, 0);

fn key(tick: u64) -> BirthKey {
    BirthKey::new(tick, SourceKind::Spawn, 0, 0)
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
        .map(|entry| (entry.target.tick(), entry.writer, entry.sequence))
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

#[test]
fn an_overlay_reads_removed_blocks_as_air_even_where_the_base_has_masks() {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let mut world = TestWorld::new();
    world.fill(BlockPos::new(0, 0, 0), BlockPos::new(2, 0, 2), blocks.stone);
    world.build_masks(&blocks.table);
    let mut removed = FxHashMap::default();
    removed.insert(BlockPos::new(1, 0, 1), BlockStateId::AIR);
    let overlay = BlockOverlay {
        base: &world,
        blocks: &removed,
    };
    let entity = QueryEntity {
        key: key(0),
        ..TestFixture::query_entity(DVec3::ZERO)
    };
    let inside = |x: f64| Aabb::new(DVec3::new(x, 0.2, 1.2), DVec3::new(x + 0.6, 0.8, 1.8));
    let mut found = Vec::new();
    let base = View::new(&world, &NoBodies, &tables);
    let over = View::new(&overlay, &NoBodies, &tables);
    assert!(
        intersects(base, &entity, inside(1.2), &mut found),
        "the base still holds the block"
    );
    assert!(
        !intersects(over, &entity, inside(1.2), &mut found),
        "the overlay removed it"
    );
    assert!(
        intersects(over, &entity, inside(0.2), &mut found),
        "the overlay keeps the other blocks"
    );
}

fn collider(x: f64) -> EntityCollider {
    EntityCollider {
        bounds: Aabb::new(DVec3::new(x, 0.0, 0.0), DVec3::new(x + 1.0, 1.0, 1.0)),
        id: EntityId(0),
    }
}

#[test]
fn a_tile_is_ready_only_when_every_group_that_touches_it_finished() {
    let mut ends = ColliderEnds::default();
    let quiet = TileKey::new(5, 0, 0);
    ends.expect(TILE);
    ends.expect(TILE);
    assert_eq!(ends.ready(quiet), Some(&[][..]), "no group touches it");
    assert_eq!(ends.ready(TILE), None);
    ends.finish(TILE, [collider(0.0)]);
    assert_eq!(ends.ready(TILE), None, "one group is still stepping");
    ends.finish(TILE, [collider(2.0)]);
    assert_eq!(ends.ready(TILE), Some(&[collider(0.0), collider(2.0)][..]));
    ends.clear();
    assert_eq!(ends.ready(TILE), Some(&[][..]));
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
fn the_pass_writes_the_back_buffer_and_the_freeze_swaps_only_written_tiles() {
    let mut grid = BodyGrid::default();
    let quiet = TileKey::new(1, 0, 0);
    grid.back_mut(TILE).push(grid_row(1.0));
    grid.back_mut(quiet).push(grid_row(9.0));
    grid.swap();

    grid.back_mut(TILE).push(grid_row(2.0));
    assert_eq!(
        grid.front(TILE),
        &[grid_row(1.0)],
        "the pass reads the old front"
    );
    grid.swap();
    assert_eq!(
        grid.front(TILE),
        &[grid_row(2.0)],
        "the back starts empty each tick"
    );
    assert_eq!(
        grid.front(quiet),
        &[grid_row(9.0)],
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
    let mut rows = TileRows::default();
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
