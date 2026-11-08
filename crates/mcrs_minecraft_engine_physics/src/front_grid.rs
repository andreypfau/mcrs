use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine::player_tiles::TILE_WIDTH;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::TileBodies;
use mcrs_minecraft_physics::floor_block;

use crate::tile_at::tile_at;

/// The edge of a bucket in blocks; a tile holds `BUCKETS × BUCKETS` buckets.
const BUCKET: i32 = 4;
const BUCKETS: usize = (TILE_WIDTH.get() / BUCKET.unsigned_abs()) as usize;

/// The front buffer of the body grid as the bodies that a step reads: the rows of each tile sorted
/// by bucket, so a query reads only the buckets that its box can reach.
#[derive(Clone, Copy, Debug)]
pub struct FrontGrid<'a> {
    pub grid: &'a BodyGrid,
    /// The largest half width of a body in the grid.
    pub reach: f32,
}

impl BodyView for FrontGrid<'_> {
    fn colliders(&self, _query: Aabb, _mover: BirthKey, _out: &mut Vec<EntityCollider>) {}

    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>) {
        let reach = f64::from(of.half_width) + f64::from(self.reach);
        let low = of.position - DVec3::new(reach, 0.0, reach);
        let high = of.position + DVec3::new(reach, 0.0, reach);
        let (first, last) = (tile_at(low), tile_at(high));
        for tile_z in first.z..=last.z {
            for tile_x in first.x..=last.x {
                let tile = TileKey::new(tile_x, 0, tile_z);
                let bodies = self.grid.front(tile);
                let [x0, z0] = bucket_xz(tile, low);
                let [x1, z1] = bucket_xz(tile, high);
                for z in z0..=z1 {
                    out.extend_from_slice(bodies.cells(z * BUCKETS + x0, z * BUCKETS + x1));
                }
            }
        }
    }
}

/// The grid row of a living row: every living entity is a body that pushes and can be pushed.
#[must_use]
pub fn grid_row(state: &EntityState, motion: &MotionRow, id: EntityId) -> Option<GridRow> {
    let living = state.living?;
    let dimensions = motion.dimensions.scaled(living.attributes.scale as f32);
    Some(GridRow {
        position: state.body.position,
        half_width: dimensions.width / 2.0,
        height: dimensions.height,
        key: state.key,
        id,
        root_vehicle: id,
        team: TeamSlot::default(),
        flags: GridFlags::PUSHABLE,
        push_targets: PushTargets::Each,
    })
}

/// Writes `bodies` into `out` in the order of the front buffer of `tile`: by bucket, then by
/// birth key.
pub fn sort_bodies(tile: TileKey, bodies: &[GridRow], out: &mut TileBodies) {
    let mut order: Vec<(usize, BirthKey, usize)> = bodies
        .iter()
        .enumerate()
        .map(|(index, row)| (bucket(tile, row.position), row.key, index))
        .collect();
    order.sort_unstable();
    out.rows.clear();
    out.rows
        .extend(order.iter().filter_map(|&(.., index)| bodies.get(index)));
    out.starts.clear();
    let mut at = 0;
    for bucket in 0..=BUCKETS * BUCKETS {
        out.starts.push(at);
        while order.get(at).is_some_and(|&(of, ..)| of == bucket) {
            at += 1;
        }
    }
}

fn bucket(tile: TileKey, position: DVec3) -> usize {
    let [x, z] = bucket_xz(tile, position);
    z * BUCKETS + x
}

/// The bucket of `position` in `tile` on `x` and `z`, clamped to the tile.
fn bucket_xz(tile: TileKey, position: DVec3) -> [usize; 2] {
    let width = TILE_WIDTH.get().cast_signed();
    let local = |value: f64, origin: i32| {
        let block = floor_block(value).saturating_sub(origin.saturating_mul(width));
        usize::try_from(block / BUCKET).map_or(0, |bucket| bucket.min(BUCKETS - 1))
    };
    [local(position.x, tile.x), local(position.z, tile.z)]
}
