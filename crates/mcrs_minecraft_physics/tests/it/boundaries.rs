use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BlockOverlay;
use mcrs_minecraft_physics::ColliderEnds;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::QueryEntity;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::intersects;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

const TILE: TileKey = TileKey::new(0, 0, 0);

fn key(tick: u64) -> EntityKey {
    EntityKey(u128::from(tick))
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
