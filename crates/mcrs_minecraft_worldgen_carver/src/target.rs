use crate::mask::CarvingMask;
use crate::modern::SOURCE_RADIUS;
use crate::tunnel::can_reach;
use crate::water::WaterMask;
use crate::{CarveShape, carve_ellipsoid};

pub trait CarveTarget {
    type Live: Copy;

    fn live(&self, source_x: i32, source_z: i32) -> Self::Live;

    fn reach(
        &self,
        live: Self::Live,
        x: f64,
        z: f64,
        step: i32,
        total_steps: i32,
        thickness: f32,
    ) -> Option<Self::Live>;

    #[allow(clippy::too_many_arguments)]
    fn carve(
        &mut self,
        live: Self::Live,
        x: f64,
        y: f64,
        z: f64,
        horizontal_radius: f64,
        vertical_radius: f64,
        shape: CarveShape<'_>,
    ) -> bool;
}

pub struct SingleColumn<'a> {
    chunk_x: i32,
    chunk_z: i32,
    water: &'a WaterMask,
    mask: &'a mut CarvingMask,
}

impl<'a> SingleColumn<'a> {
    pub fn new(
        chunk_x: i32,
        chunk_z: i32,
        water: &'a WaterMask,
        mask: &'a mut CarvingMask,
    ) -> Self {
        SingleColumn {
            chunk_x,
            chunk_z,
            water,
            mask,
        }
    }
}

impl CarveTarget for SingleColumn<'_> {
    type Live = ();

    #[inline]
    fn live(&self, _source_x: i32, _source_z: i32) {}

    #[inline]
    fn reach(
        &self,
        _live: (),
        x: f64,
        z: f64,
        step: i32,
        total_steps: i32,
        thickness: f32,
    ) -> Option<()> {
        can_reach(
            self.chunk_x,
            self.chunk_z,
            x,
            z,
            step,
            total_steps,
            thickness,
        )
        .then_some(())
    }

    #[inline]
    fn carve(
        &mut self,
        _live: (),
        x: f64,
        y: f64,
        z: f64,
        horizontal_radius: f64,
        vertical_radius: f64,
        shape: CarveShape<'_>,
    ) -> bool {
        carve_ellipsoid(
            self.chunk_x,
            self.chunk_z,
            x,
            y,
            z,
            horizontal_radius,
            vertical_radius,
            shape,
            self.water,
            self.mask,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Columns(u64);

impl Columns {
    #[inline]
    fn contains(self, slot: usize) -> bool {
        (self.0 >> slot) & 1 != 0
    }
}

/// A square of chunk columns carved by one walk per source. Slot `i` is the
/// column at `origin_x + i / width`, `origin_z + i % width`.
pub struct Region<'a> {
    origin_x: i32,
    origin_z: i32,
    width: i32,
    masks: &'a mut [CarvingMask],
    no_water: WaterMask,
}

impl<'a> Region<'a> {
    pub const MAX_WIDTH: i32 = 8;

    pub fn new(origin_x: i32, origin_z: i32, width: i32, masks: &'a mut [CarvingMask]) -> Self {
        assert!(
            (1..=Self::MAX_WIDTH).contains(&width),
            "a region is at most {} columns wide, not {width}",
            Self::MAX_WIDTH
        );
        assert_eq!(masks.len(), (width * width) as usize);
        Region {
            origin_x,
            origin_z,
            width,
            masks,
            no_water: WaterMask::default(),
        }
    }

    #[inline]
    fn column_of(&self, slot: usize) -> (i32, i32) {
        let slot = slot as i32;
        (
            self.origin_x + slot / self.width,
            self.origin_z + slot % self.width,
        )
    }
}

impl CarveTarget for Region<'_> {
    type Live = Columns;

    fn live(&self, source_x: i32, source_z: i32) -> Columns {
        let mut bits = 0u64;
        for slot in 0..self.masks.len() {
            let (chunk_x, chunk_z) = self.column_of(slot);
            if (chunk_x - source_x).abs() <= SOURCE_RADIUS
                && (chunk_z - source_z).abs() <= SOURCE_RADIUS
            {
                bits |= 1 << slot;
            }
        }
        Columns(bits)
    }

    fn reach(
        &self,
        live: Columns,
        x: f64,
        z: f64,
        step: i32,
        total_steps: i32,
        thickness: f32,
    ) -> Option<Columns> {
        let mut pending = live.0;
        let mut kept = 0u64;
        while pending != 0 {
            let slot = pending.trailing_zeros() as usize;
            pending &= pending - 1;
            let (chunk_x, chunk_z) = self.column_of(slot);
            if can_reach(chunk_x, chunk_z, x, z, step, total_steps, thickness) {
                kept |= 1 << slot;
            }
        }
        (kept != 0).then_some(Columns(kept))
    }

    fn carve(
        &mut self,
        live: Columns,
        x: f64,
        y: f64,
        z: f64,
        horizontal_radius: f64,
        vertical_radius: f64,
        shape: CarveShape<'_>,
    ) -> bool {
        let (first_x, last_x) = chunks_touched(x, horizontal_radius);
        let (first_z, last_z) = chunks_touched(z, horizontal_radius);
        let first_x = first_x.max(self.origin_x);
        let last_x = last_x.min(self.origin_x + self.width - 1);
        let first_z = first_z.max(self.origin_z);
        let last_z = last_z.min(self.origin_z + self.width - 1);

        let mut carved = false;
        for chunk_x in first_x..=last_x {
            for chunk_z in first_z..=last_z {
                let slot =
                    ((chunk_x - self.origin_x) * self.width + (chunk_z - self.origin_z)) as usize;
                if live.contains(slot) {
                    carved |= carve_ellipsoid(
                        chunk_x,
                        chunk_z,
                        x,
                        y,
                        z,
                        horizontal_radius,
                        vertical_radius,
                        shape,
                        &self.no_water,
                        &mut self.masks[slot],
                    );
                }
            }
        }
        carved
    }
}

/// The chunks along one axis in which `carve_ellipsoid`'s clipped bounds are not empty.
#[inline]
fn chunks_touched(center: f64, horizontal_radius: f64) -> (i32, i32) {
    let first = (center - horizontal_radius).floor() as i32 - 1;
    let last = (center + horizontal_radius).floor() as i32;
    (first >> 4, last >> 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CarverConfig;
    use crate::modern::carve_source_into;
    use crate::tunnel::{SplitSeeding, TrigIndex, TunnelShape, walk_tunnel_into};
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_core::value_provider::HeightContext;
    use mcrs_minecraft_random::Random;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_worldgen_testing::read;

    fn overworld() -> HeightContext {
        HeightContext {
            min_y: -64,
            depth: 384,
            sea_level: 63,
        }
    }

    fn empty_mask() -> CarvingMask {
        CarvingMask::new(-63, 312)
    }

    fn masks(width: i32) -> Vec<CarvingMask> {
        (0..width * width).map(|_| empty_mask()).collect()
    }

    fn shipped(name: &str) -> CarverConfig {
        read("carver", &ResourceLocation::minecraft(name).unwrap())
    }

    fn compare_region_with_columns(
        config: &CarverConfig,
        index: usize,
        seed: i64,
        width: i32,
        origin: (i32, i32),
    ) -> usize {
        let mut slots = masks(width);
        let mut region = Region::new(origin.0, origin.1, width, &mut slots);
        for source_x in origin.0 - SOURCE_RADIUS..=origin.0 + width - 1 + SOURCE_RADIUS {
            for source_z in origin.1 - SOURCE_RADIUS..=origin.1 + width - 1 + SOURCE_RADIUS {
                carve_source_into(
                    config,
                    index,
                    seed,
                    overworld(),
                    &mut region,
                    source_x,
                    source_z,
                );
            }
        }

        let mut carved = 0;
        for (slot, region_mask) in slots.iter().enumerate() {
            let column = (
                origin.0 + slot as i32 / width,
                origin.1 + slot as i32 % width,
            );
            let mut expected = empty_mask();
            let water = WaterMask::default();
            let mut alone = SingleColumn::new(column.0, column.1, &water, &mut expected);
            for source_x in column.0 - SOURCE_RADIUS..=column.0 + SOURCE_RADIUS {
                for source_z in column.1 - SOURCE_RADIUS..=column.1 + SOURCE_RADIUS {
                    carve_source_into(
                        config,
                        index,
                        seed,
                        overworld(),
                        &mut alone,
                        source_x,
                        source_z,
                    );
                }
            }
            assert!(
                *region_mask == expected,
                "slot {slot} (column {column:?}) of a width {width} region at {origin:?}, seed {seed}, \
                 carver {index} differs from the column carved on its own"
            );
            carved += usize::from(!expected.is_empty());
        }
        carved
    }

    const CARVERS: [&str; 3] = ["cave", "cave_extra_underground", "canyon"];

    #[test]
    fn a_region_slot_equals_its_columns_mask() {
        let mut carved = [0usize; 3];
        for (index, name) in CARVERS.iter().enumerate() {
            let config = shipped(name);
            for (width, origin) in [(3, (-1, -2)), (8, (-4, 0))] {
                carved[index] += compare_region_with_columns(&config, index, 12345, width, origin);
            }
        }
        assert!(
            carved.iter().all(|&slots| slots > 0),
            "a carver left every slot empty, so nothing was compared: {carved:?}"
        );
    }

    #[test]
    fn a_source_past_a_columns_radius_does_not_carve_it() {
        let mut slots = masks(2);
        let region = Region::new(0, 0, 2, &mut slots);
        assert_eq!(region.live(8, 0).0, 0b1111);
        assert_eq!(region.live(9, 0).0, 0b1100);
        assert_eq!(region.live(-8, 0).0, 0b0011);
        assert_eq!(region.live(0, 9).0, 0b1010);
        assert_eq!(region.live(10, 0).0, 0);

        let walk = |seed: u64, live: Columns| {
            let mut slots = masks(2);
            let mut region = Region::new(0, 0, 2, &mut slots);
            let mut tunnel_rng = LegacyRandom::new(seed);
            let mut parent = LegacyRandom::new(8);
            walk_tunnel_into(
                &mut region,
                live,
                152.0,
                40.0,
                8.0,
                TunnelShape {
                    thickness: 1.0,
                    y_scale: 1.0,
                    horizontal_radius_multiplier: 1.0,
                    vertical_radius_multiplier: 1.0,
                    trig: TrigIndex::Modern,
                },
                std::f32::consts::PI,
                0.0,
                0,
                300,
                false,
                SplitSeeding::FromTunnel,
                CarveShape::Cave { floor_level: -0.7 },
                &mut tunnel_rng,
                &mut parent,
            );
            drop(region);
            slots
        };

        let (mut reaches_live, mut would_reach_far) = (0, 0);
        for seed in 0..64 {
            let held_back = walk(seed, Columns(0b1100));
            assert!(
                held_back[0].is_empty() && held_back[1].is_empty(),
                "seed {seed}"
            );
            reaches_live += usize::from(!held_back[2].is_empty() || !held_back[3].is_empty());
            let unrestricted = walk(seed, Columns(0b1111));
            would_reach_far += usize::from(!unrestricted[0].is_empty());
        }
        assert!(
            reaches_live > 0,
            "no tunnel reached the columns that are live for it"
        );
        assert!(
            would_reach_far > 0,
            "no tunnel would have reached the column held back"
        );
    }

    #[test]
    fn a_region_of_one_column_is_that_column() {
        let shape = TunnelShape {
            thickness: 3.0,
            y_scale: 1.0,
            horizontal_radius_multiplier: 1.0,
            vertical_radius_multiplier: 1.0,
            trig: TrigIndex::Modern,
        };
        let mut carved = 0;
        for seed in 0..24u64 {
            let (mut a_rng, mut a_parent) = (LegacyRandom::new(seed), LegacyRandom::new(1));
            let (mut b_rng, mut b_parent) = (LegacyRandom::new(seed), LegacyRandom::new(1));
            let water = WaterMask::default();
            let mut single_mask = empty_mask();
            let mut single = SingleColumn::new(-1, 0, &water, &mut single_mask);
            walk_tunnel_into(
                &mut single,
                (),
                -9.0 + seed as f64,
                40.0,
                8.0,
                shape,
                seed as f32,
                0.0,
                0,
                100,
                false,
                SplitSeeding::FromTunnel,
                CarveShape::Cave { floor_level: -0.7 },
                &mut a_rng,
                &mut a_parent,
            );

            let mut slots = masks(1);
            let mut region = Region::new(-1, 0, 1, &mut slots);
            let live = region.live(-1, 0);
            walk_tunnel_into(
                &mut region,
                live,
                -9.0 + seed as f64,
                40.0,
                8.0,
                shape,
                seed as f32,
                0.0,
                0,
                100,
                false,
                SplitSeeding::FromTunnel,
                CarveShape::Cave { floor_level: -0.7 },
                &mut b_rng,
                &mut b_parent,
            );
            drop(region);

            assert!(slots[0] == single_mask, "seed {seed}");
            assert_eq!(a_rng.next_java_long(), b_rng.next_java_long());
            carved += usize::from(!single_mask.is_empty());
        }
        assert!(carved > 0, "no walk marked anything");
    }

    #[test]
    fn a_walk_no_column_can_reach_ends_at_once() {
        let total_steps = 100;
        for seed in 0..16u64 {
            let mut slots = masks(2);
            let mut region = Region::new(0, 0, 2, &mut slots);
            let live = region.live(0, 0);
            let mut tunnel_rng = LegacyRandom::new(seed);
            let mut parent = LegacyRandom::new(99);
            walk_tunnel_into(
                &mut region,
                live,
                100_000.0,
                40.0,
                8.0,
                TunnelShape {
                    thickness: 3.0,
                    y_scale: 1.0,
                    horizontal_radius_multiplier: 1.0,
                    vertical_radius_multiplier: 1.0,
                    trig: TrigIndex::Modern,
                },
                0.0,
                0.0,
                0,
                total_steps,
                false,
                SplitSeeding::FromTunnel,
                CarveShape::Cave { floor_level: -0.7 },
                &mut tunnel_rng,
                &mut parent,
            );
            drop(region);
            assert!(slots.iter().all(CarvingMask::is_empty));
            assert_eq!(
                parent.next_java_long(),
                LegacyRandom::new(99).next_java_long()
            );

            let mut replay = LegacyRandom::new(seed);
            replay.next_i32_bound(total_steps / 2);
            replay.next_i32_bound(6);
            loop {
                for _ in 0..6 {
                    replay.next_f32();
                }
                if replay.next_i32_bound(4) != 0 {
                    break;
                }
            }
            assert_eq!(
                tunnel_rng.next_java_long(),
                replay.next_java_long(),
                "seed {seed}: the walk drew past the first step that tests reach"
            );
        }
    }

    mod exhaustive {
        use super::*;

        #[test]
        fn a_region_slot_equals_its_columns_mask_at_every_width_and_origin() {
            let mut carved = [0usize; 3];
            for (index, name) in CARVERS.iter().enumerate() {
                let config = shipped(name);
                for seed in [12345, 845, 7] {
                    for width in [1, 2, 3, 4, 8] {
                        for origin in [-width, -1, 0] {
                            carved[index] += compare_region_with_columns(
                                &config,
                                index,
                                seed,
                                width,
                                (origin, 3 - origin),
                            );
                        }
                    }
                }
            }
            assert!(carved.iter().all(|&slots| slots > 0), "{carved:?}");
        }
    }
}
