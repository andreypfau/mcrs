use core::mem;

use glam::DVec3;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use rustc_hash::FxHashMap;

use crate::Aabb;
use crate::BodyView;
use crate::EntityCollider;
use crate::GridRow;
use crate::TileBodies;
use crate::tile_bodies::BUCKETS;
use crate::tile_bodies::NO_BODIES;
use crate::tile_bodies::bucket_xz;

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct BodyGrid {
    tiles: FxHashMap<TileKey, TileBodies>,
    reach: f32,
}

impl BodyGrid {
    #[must_use]
    pub fn tile(&self, tile: TileKey) -> &TileBodies {
        self.tiles.get(&tile).unwrap_or(&NO_BODIES)
    }

    pub fn swap(&mut self, tile: TileKey, back: &mut TileBodies) {
        mem::swap(self.tiles.entry(tile).or_default(), back);
    }

    pub fn tiles(&self) -> impl Iterator<Item = (TileKey, &TileBodies)> {
        self.tiles.iter().map(|(tile, bodies)| (*tile, bodies))
    }

    pub fn in_box(&self, min: DVec3, max: DVec3) -> impl Iterator<Item = &GridRow> {
        let first = TileKey::at(min);
        let last = TileKey::at(max);
        (first.x..=last.x).flat_map(move |x| {
            (first.z..=last.z).flat_map(move |z| {
                let tile = TileKey::new(x, 0, z);
                self.tile(tile).in_box(tile, min, max)
            })
        })
    }

    pub fn widen(&mut self, reach: f32) {
        self.reach = self.reach.max(reach);
    }
}

impl BodyView for BodyGrid {
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
                let bodies = self.tile(tile);
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
