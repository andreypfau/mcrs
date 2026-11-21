use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::TeamSlot;

use crate::BodyGrid;
use crate::TileBodies;
use crate::tile_bodies::{BUCKETS, bucket_xz};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrontGrid<'a> {
    pub(crate) grid: &'a BodyGrid,
    pub(crate) reach: f32,
}

impl BodyView for FrontGrid<'_> {
    fn colliders(&self, _query: Aabb, _mover: EntityUuid, _out: &mut Vec<EntityCollider>) {}

    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>) {
        let reach = f64::from(of.half_width) + f64::from(self.reach);
        let low = of.position - DVec3::new(reach, 0.0, reach);
        let high = of.position + DVec3::new(reach, 0.0, reach);
        let bounds = of.bounds();
        let (first, last) = (TileKey::at(low), TileKey::at(high));
        for tile_z in first.z..=last.z {
            for tile_x in first.x..=last.x {
                let tile = TileKey::new(tile_x, 0, tile_z);
                let bodies = self.grid.front(tile);
                let [x0, z0] = bucket_xz(tile, low);
                let [x1, z1] = bucket_xz(tile, high);
                for z in z0..=z1 {
                    out.extend(
                        bodies
                            .cells(z * BUCKETS + x0, z * BUCKETS + x1)
                            .iter()
                            .filter(|row| row.bounds().intersects(&bounds)),
                    );
                }
            }
        }
    }
}

#[must_use]
pub(crate) fn grid_row(state: &EntityState, motion: &MotionRow, id: EntityId) -> Option<GridRow> {
    let living = state.living?;
    let dimensions = motion.dimensions.scaled(living.attributes.scale as f32);
    let position = state.body.position;
    let previous = if state.last_path.end == position {
        state.last_path.start
    } else {
        position
    };
    Some(GridRow {
        position,
        previous,
        half_width: dimensions.width / 2.0,
        height: dimensions.height,
        key: state.key,
        id,
        root_vehicle: id,
        team: TeamSlot::default(),
        flags: if living.is_alive() {
            GridFlags::PUSHABLE
        } else {
            GridFlags::empty()
        },
        push_targets: PushTargets::Each,
    })
}

pub(crate) fn sort_bodies(tile: TileKey, bodies: &[GridRow], out: &mut TileBodies) {
    let mut order: Vec<(usize, EntityUuid, usize)> = bodies
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
