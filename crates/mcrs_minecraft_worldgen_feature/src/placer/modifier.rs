use std::sync::LazyLock;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::placement::{HeightmapName, PlacementModifier, VerticalDirection};
use mcrs_minecraft_worldgen_noise::simplex::SimplexNoise;

use super::{Predicate, WorldGenVolume};

/// The reference's `Biome.BIOME_INFO_NOISE`: seeded 2345, no world seed in it,
/// so every world's flower and dripstone counts follow the same field.
static BIOME_INFO_NOISE: LazyLock<SimplexNoise> =
    LazyLock::new(|| SimplexNoise::from_random_at_origin(&mut LegacyRandom::new(2345)));

/// A placement modifier with every set it names reduced to a mask.
pub type Modifier = PlacementModifier<Predicate>;

pub fn biome_info_noise(x: f64, z: f64) -> f64 {
    BIOME_INFO_NOISE.sample_2d(x, z, 1.0, 1.0) as f32 as f64
}

impl PlacementModifier<Predicate> {
    pub(super) fn apply<W: WorldGenVolume, R: Random>(
        &self,
        volume: &W,
        rng: &mut R,
        origin: BlockPos,
        carries: &dyn Fn(u32) -> bool,
        out: &mut Vec<BlockPos>,
    ) {
        use PlacementModifier::*;
        match self {
            BlockPredicateFilter { predicate } => {
                if predicate.test(volume, origin) {
                    out.push(origin);
                }
            }
            RarityFilter { chance } => {
                if rng.next_f32() < 1.0 / chance.0 as f32 {
                    out.push(origin);
                }
            }
            RandomChance { chance } => {
                if rng.next_f32() < chance.0 as f32 {
                    out.push(origin);
                }
            }
            SurfaceRelativeThresholdFilter {
                heightmap,
                min_inclusive,
                max_inclusive,
            } => {
                let surface = volume.height(*heightmap, origin.x, origin.z) as i64;
                let y = origin.y as i64;
                if surface + min_inclusive.0 as i64 <= y && y <= surface + max_inclusive.0 as i64 {
                    out.push(origin);
                }
            }
            SurfaceWaterDepthFilter { max_water_depth } => {
                let floor = volume.height(HeightmapName::OceanFloor, origin.x, origin.z);
                let surface = volume.height(HeightmapName::WorldSurface, origin.x, origin.z);
                if surface - floor <= *max_water_depth {
                    out.push(origin);
                }
            }
            Biome {} => {
                if carries(volume.biome(origin)) {
                    out.push(origin);
                }
            }
            Count { count } => {
                out.extend(std::iter::repeat_n(
                    origin,
                    count.sample(rng).max(0) as usize,
                ));
            }
            NoiseBasedCount {
                noise_to_count_ratio,
                noise_factor,
                noise_offset,
            } => {
                let noise = biome_info_noise(
                    origin.x as f64 / *noise_factor,
                    origin.z as f64 / *noise_factor,
                );
                let count = ((noise + *noise_offset) * *noise_to_count_ratio as f64).ceil() as i32;
                out.extend(std::iter::repeat_n(origin, count.max(0) as usize));
            }
            NoiseThresholdCount {
                noise_level,
                below_noise,
                above_noise,
            } => {
                let noise = biome_info_noise(origin.x as f64 / 200.0, origin.z as f64 / 200.0);
                let count = if noise < *noise_level {
                    *below_noise
                } else {
                    *above_noise
                };
                out.extend(std::iter::repeat_n(origin, count.max(0) as usize));
            }
            CountOnEveryLayer { count } => {
                let mut layer = 0;
                loop {
                    let mut found_any = false;
                    let mut i = 0;
                    while i < count.sample(rng) {
                        let x = rng.next_i32_bound(16) + origin.x;
                        let z = rng.next_i32_bound(16) + origin.z;
                        let start = volume.height(HeightmapName::MotionBlocking, x, z);
                        if let Some(y) = on_ground_y(volume, x, start, z, layer) {
                            out.push(BlockPos::new(x, y, z));
                            found_any = true;
                        }
                        i += 1;
                    }
                    layer += 1;
                    if !found_any {
                        break;
                    }
                }
            }
            Cuboid {
                xz_size,
                y_size,
                include_edges,
                include_interior,
            } => {
                let height = y_size.sample(rng);
                let width = xz_size.sample(rng);
                let length = xz_size.sample(rng);
                for x in 0..=width {
                    for y in 0..=height {
                        for z in 0..=length {
                            let edge_xy =
                                *include_edges || (x != 0 && x != width) || (y != 0 && y != height);
                            let edge_zy = *include_edges
                                || (z != 0 && z != length)
                                || (y != 0 && y != height);
                            let edge_xz =
                                *include_edges || (x != 0 && x != width) || (z != 0 && z != length);
                            let interior = *include_interior
                                || x == 0
                                || x == width
                                || y == 0
                                || y == height
                                || z == 0
                                || z == length;
                            if edge_xy && edge_zy && edge_xz && interior {
                                out.push(BlockPos::new(x + origin.x, y + origin.y, z + origin.z));
                            }
                        }
                    }
                }
            }
            EnvironmentScan {
                direction_of_search,
                target_condition,
                allowed_search_condition,
                max_steps,
            } => {
                let allowed = |pos| {
                    allowed_search_condition
                        .as_ref()
                        .is_none_or(|p| p.test(volume, pos))
                };
                if !allowed(origin) {
                    return;
                }
                let step = match direction_of_search {
                    VerticalDirection::Up => 1,
                    VerticalDirection::Down => -1,
                };
                let extent = volume.extent();
                let mut pos = origin;
                for _ in 0..max_steps.0 {
                    if target_condition.test(volume, pos) {
                        out.push(pos);
                        return;
                    }
                    pos.y += step;
                    if !extent.contains(pos.y) {
                        return;
                    }
                    if !allowed(pos) {
                        break;
                    }
                }
                if target_condition.test(volume, pos) {
                    out.push(pos);
                }
            }
            Heightmap { heightmap } => {
                let height = volume.height(*heightmap, origin.x, origin.z);
                if height > volume.extent().min_y {
                    out.push(BlockPos::new(origin.x, height, origin.z));
                }
            }
            HeightRange { height } => {
                let y = height.sample(rng, volume.extent());
                out.push(BlockPos::new(origin.x, y, origin.z));
            }
            InSquare {} => {
                let x = rng.next_i32_bound(16) + origin.x;
                let z = rng.next_i32_bound(16) + origin.z;
                out.push(BlockPos::new(x, origin.y, z));
            }
            Offset { x, y, z } => {
                let dx = x.sample(rng);
                let dy = y.sample(rng);
                let dz = z.sample(rng);
                out.push(BlockPos::new(origin.x + dx, origin.y + dy, origin.z + dz));
            }
            RandomlySelected { placements } => {
                let chosen = rng.next_i32_bound(placements.len() as i32) as usize;
                placements[chosen].apply(volume, rng, origin, carries, out);
            }
            FixedPlacement { positions } => {
                let chunk_x = origin.x >> 4;
                let chunk_z = origin.z >> 4;
                for &position in positions {
                    let position = BlockPos::from(position);
                    if position.x >> 4 == chunk_x && position.z >> 4 == chunk_z {
                        out.push(position);
                    }
                }
            }
        }
    }
}

/// `CountOnEveryLayerPlacement.findOnGroundYPosition`: the first solid top face
/// below the start that is the `layer`-th one down. Empty is air, water or lava.
fn on_ground_y<W: WorldGenVolume>(
    volume: &W,
    x: i32,
    y_start: i32,
    z: i32,
    layer_to_place_on: i32,
) -> Option<i32> {
    let world = volume.world();
    let is_empty = |state: VoxelId| world.is_empty_or_water_or_lava(state);
    let bedrock = &world.bedrock;
    let min_y = volume.extent().min_y;
    let mut current_layer = 0;
    let mut current = volume.get(BlockPos::new(x, y_start, z));
    let mut y = y_start;
    while y >= min_y + 1 {
        let below = volume.get(BlockPos::new(x, y - 1, z));
        if !is_empty(below) && is_empty(current) && !bedrock.contains(below.0 as usize) {
            if current_layer == layer_to_place_on {
                return Some(y);
            }
            current_layer += 1;
        }
        current = below;
        y -= 1;
    }
    None
}
