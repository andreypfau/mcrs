use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
use mcrs_minecraft_worldgen::feature::placer::{Rule, WorldGenVolume};
use mcrs_voxel_math::Direction;
use mcrs_voxel_math::mth::{lerp, sin_modern};
use mcrs_voxel_storage::VoxelId;

#[derive(Clone, Debug)]
pub struct OreReplacement {
    pub target: Rule,
    pub state: VoxelId,
}

#[derive(Clone, Debug)]
pub struct CompiledOre {
    pub targets: Vec<OreReplacement>,
    pub size: i32,
    pub discard_chance_on_air_exposure: f32,
}

/// The two buffers a vein fills and drops again: the sphere table and the set
/// of positions already visited. A column places hundreds of veins, so these
/// are refilled from the run rather than allocated per vein.
#[derive(Default)]
pub struct OreScratch {
    spheres: Vec<[f64; 4]>,
    tested: FixedBitSet,
}

/// The modern ore vein: `size` sphere centres along a segment, the pairwise cull
/// of a sphere swallowed by a larger one, and one visit per position across all
/// spheres.
///
/// The three draws of the segment are spent before the heightmap probe, so a
/// vein that finds nothing above `y_start` still advances the source.
pub fn place_modern_ore<W: WorldGenVolume>(
    cfg: &CompiledOre,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: IVec3,
    scratch: &mut OreScratch,
) -> bool {
    let (origin_x, origin_y, origin_z) = (origin.x, origin.y, origin.z);
    let size = cfg.size;
    let dir = rng.next_f32() * std::f32::consts::PI;
    let spread_xy = size as f32 / 8.0;
    let max_radius = (((size as f32 / 16.0 * 2.0 + 1.0) / 2.0) as f64).ceil() as i32;
    let sin_dir = (dir as f64).sin();
    let cos_dir = (dir as f64).cos();
    let x0 = origin_x as f64 + sin_dir * spread_xy as f64;
    let x1 = origin_x as f64 - sin_dir * spread_xy as f64;
    let z0 = origin_z as f64 + cos_dir * spread_xy as f64;
    let z1 = origin_z as f64 - cos_dir * spread_xy as f64;
    let y0 = (origin_y + rng.next_i32_bound(3) - 2) as f64;
    let y1 = (origin_y + rng.next_i32_bound(3) - 2) as f64;

    let ceil_spread = (spread_xy as f64).ceil() as i32;
    let x_start = origin_x - ceil_spread - max_radius;
    let y_start = origin_y - 2 - max_radius;
    let z_start = origin_z - ceil_spread - max_radius;
    let size_xz = 2 * (ceil_spread + max_radius);
    let size_y = 2 * (2 + max_radius);

    for x_probe in x_start..=x_start + size_xz {
        for z_probe in z_start..=z_start + size_xz {
            if y_start <= volume.height(HeightmapName::OceanFloorWg, x_probe, z_probe) {
                return do_place(
                    cfg,
                    volume,
                    rng,
                    [x0, x1, z0, z1, y0, y1],
                    [x_start, y_start, z_start],
                    size_xz,
                    size_y,
                    scratch,
                );
            }
        }
    }
    false
}

fn do_place<W: WorldGenVolume>(
    cfg: &CompiledOre,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    [x0, x1, z0, z1, y0, y1]: [f64; 6],
    [x_start, y_start, z_start]: [i32; 3],
    size_xz: i32,
    size_y: i32,
    scratch: &mut OreScratch,
) -> bool {
    let size = cfg.size;
    let mut placed = 0u32;
    let OreScratch { spheres, tested } = scratch;
    tested.clear();
    tested.grow(bit_set_capacity(size_xz, size_y));
    spheres.clear();

    for i in 0..size.max(0) {
        let step = i as f32 / size as f32;
        let alpha = step as f64;
        let scale = rng.next_f64() * size as f64 / 16.0;
        let bulge = (sin_modern((std::f32::consts::PI * step) as f64) + 1.0) as f64;
        spheres.push([
            lerp(alpha, x0, x1),
            lerp(alpha, y0, y1),
            lerp(alpha, z0, z1),
            (bulge * scale + 1.0) / 2.0,
        ]);
    }

    for a in 0..spheres.len().saturating_sub(1) {
        if spheres[a][3] <= 0.0 {
            continue;
        }
        for b in a + 1..spheres.len() {
            if spheres[b][3] <= 0.0 {
                continue;
            }
            let dx = spheres[a][0] - spheres[b][0];
            let dy = spheres[a][1] - spheres[b][1];
            let dz = spheres[a][2] - spheres[b][2];
            let dr = spheres[a][3] - spheres[b][3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    spheres[b][3] = -1.0;
                } else {
                    spheres[a][3] = -1.0;
                }
            }
        }
    }

    for [xx, yy, zz, r] in spheres.iter().copied() {
        if r < 0.0 {
            continue;
        }
        let x_min = ((xx - r).floor() as i32).max(x_start);
        let y_min = ((yy - r).floor() as i32).max(y_start);
        let z_min = ((zz - r).floor() as i32).max(z_start);
        let x_max = ((xx + r).floor() as i32).max(x_min);
        let y_max = ((yy + r).floor() as i32).max(y_min);
        let z_max = ((zz + r).floor() as i32).max(z_min);

        for x in x_min..=x_max {
            let xd = (x as f64 + 0.5 - xx) / r;
            if xd * xd >= 1.0 {
                continue;
            }
            for y in y_min..=y_max {
                let yd = (y as f64 + 0.5 - yy) / r;
                if xd * xd + yd * yd >= 1.0 {
                    continue;
                }
                for z in z_min..=z_max {
                    let zd = (z as f64 + 0.5 - zz) / r;
                    if xd * xd + yd * yd + zd * zd >= 1.0 || !volume.extent().contains(y) {
                        continue;
                    }
                    let index = (x - x_start) as usize
                        + (y - y_start) as usize * size_xz as usize
                        + (z - z_start) as usize * size_xz as usize * size_y as usize;
                    if index >= tested.len() {
                        tested.grow(index + 1);
                    }
                    if tested.put(index) {
                        continue;
                    }
                    let current = volume.get(IVec3::new(x, y, z));
                    for target in &cfg.targets {
                        if can_place_ore(cfg, volume, rng, target, current, [x, y, z]) {
                            volume.set(IVec3::new(x, y, z), target.state);
                            placed += 1;
                            break;
                        }
                    }
                }
            }
        }
    }

    placed > 0
}

/// The reference's `BitSet` grows on demand; the box the spheres are clamped
/// into bounds the largest index it ever reaches.
fn bit_set_capacity(size_xz: i32, size_y: i32) -> usize {
    let (xz, y) = (size_xz.max(0) as usize, size_y.max(0) as usize);
    xz + y * xz + xz * xz * y + 1
}

pub(crate) fn can_place_ore<W: WorldGenVolume>(
    cfg: &CompiledOre,
    volume: &W,
    rng: &mut XoroshiroRandom,
    target: &OreReplacement,
    state: VoxelId,
    [x, y, z]: [i32; 3],
) -> bool {
    if !target.target.test(state, y, rng) {
        return false;
    }
    if should_skip_air_check(rng, cfg.discard_chance_on_air_exposure) {
        return true;
    }
    !is_adjacent_to_air(volume, x, y, z)
}

fn should_skip_air_check(rng: &mut XoroshiroRandom, discard_chance_on_air_exposure: f32) -> bool {
    if discard_chance_on_air_exposure <= 0.0 {
        true
    } else if discard_chance_on_air_exposure >= 1.0 {
        false
    } else {
        rng.next_f32() >= discard_chance_on_air_exposure
    }
}

fn is_adjacent_to_air<W: WorldGenVolume>(volume: &W, x: i32, y: i32, z: i32) -> bool {
    let pos = IVec3::new(x, y, z);
    Direction::all()
        .into_iter()
        .any(|direction| volume.is_air(pos + direction.normal()))
}
