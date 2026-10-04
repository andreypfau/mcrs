use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::mth::{lerp, sin_modern};
use mcrs_minecraft_core::{BlockPos, Direction};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{Rule, WorldGenVolume};

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

#[derive(Default)]
pub struct OreScratch {
    spheres: Vec<[f64; 4]>,
    column_min_y: Vec<i32>,
    column_max_y: Vec<i32>,
}

/// The three draws of the segment are spent before the heightmap probe, so a
/// vein that finds no column reaching `y_start` still advances the source.
pub fn place_modern_ore<W: WorldGenVolume>(
    cfg: &CompiledOre,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
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

    let reaches = (x_start..x_start + size_xz).any(|x| {
        (z_start..z_start + size_xz)
            .any(|z| volume.height(HeightmapName::OceanFloorWg, x, z) >= y_start)
    });
    reaches
        && do_place(
            cfg,
            volume,
            rng,
            [x0, x1, z0, z1, y0, y1],
            [x_start, z_start],
            size_xz,
            scratch,
        )
}

fn do_place<W: WorldGenVolume>(
    cfg: &CompiledOre,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    [x0, x1, z0, z1, y0, y1]: [f64; 6],
    [x_start, z_start]: [i32; 2],
    size_xz: i32,
    scratch: &mut OreScratch,
) -> bool {
    let OreScratch {
        spheres,
        column_min_y,
        column_max_y,
    } = scratch;
    let size = cfg.size;
    let mut placed = 0u32;
    spheres.clear();
    let mut largest_radius = 0.0f64;

    for i in 0..size.max(0) {
        let step = i as f32 / size as f32;
        let alpha = step as f64;
        let scale = rng.next_f64() * size as f64 / 16.0;
        let bulge = (sin_modern((std::f32::consts::PI * step) as f64) + 1.0) as f64;
        let radius = (bulge * scale + 1.0) / 2.0;
        spheres.push([
            lerp(alpha, x0, x1),
            lerp(alpha, y0, y1),
            lerp(alpha, z0, z1),
            radius,
        ]);
        largest_radius = largest_radius.max(radius);
    }

    let (dx, dy, dz) = (x1 - x0, y1 - y0, z1 - z0);
    let step_distance = (dx * dx + dy * dy + dz * dz).sqrt() / size as f64;

    for i in 0..spheres.len().saturating_sub(1) {
        let radius = spheres[i][3];
        if radius <= 0.0 {
            continue;
        }
        for j in i + 1..spheres.len() {
            let other_radius = spheres[j][3];
            if other_radius <= 0.0 {
                continue;
            }
            let distance = (j - i) as f64 * step_distance;
            let dr = radius - other_radius;
            if dr * dr > distance * distance {
                if dr <= 0.0 {
                    spheres[i][3] = -1.0;
                    break;
                }
                spheres[j][3] = -1.0;
            }
            if distance > largest_radius {
                break;
            }
        }
    }

    let extent = volume.extent();
    let lowest_y = extent.min_y;
    let highest_y = extent.min_y + extent.depth - 1;
    let grid_max_x = x_start + size_xz - 1;
    let grid_max_z = z_start + size_xz - 1;
    let columns = (size_xz.max(0) as usize).pow(2);
    column_min_y.clear();
    column_min_y.resize(columns, i32::MAX);
    column_max_y.clear();
    column_max_y.resize(columns, i32::MIN);
    let (mut touched_min_x, mut touched_max_x) = (size_xz, -1);
    let (mut touched_min_z, mut touched_max_z) = (size_xz, -1);

    for [xx, yy, zz, r] in spheres.iter().copied() {
        if r < 0.0 {
            continue;
        }
        let r_squared = r * r;
        let x_min = ((xx - r).floor() as i32).max(x_start);
        let z_min = ((zz - r).floor() as i32).max(z_start);
        let x_max = ((xx + r).floor() as i32).max(x_min).min(grid_max_x);
        let z_max = ((zz + r).floor() as i32).max(z_min).min(grid_max_z);
        touched_min_x = touched_min_x.min(x_min - x_start);
        touched_max_x = touched_max_x.max(x_max - x_start);
        touched_min_z = touched_min_z.min(z_min - z_start);
        touched_max_z = touched_max_z.max(z_max - z_start);

        for x in x_min..=x_max {
            let dx = x as f64 + 0.5 - xx;
            let remaining_after_x = r_squared - dx * dx;
            if remaining_after_x <= 0.0 {
                continue;
            }
            let row_offset = (x - x_start) * size_xz - z_start;
            for z in z_min..=z_max {
                let dz = z as f64 + 0.5 - zz;
                let remaining = remaining_after_x - dz * dz;
                if remaining <= 0.0 {
                    continue;
                }
                let half_span = remaining.sqrt();
                let y_low = ((yy - 0.5 - half_span).floor() as i32 + 1).max(lowest_y);
                let y_high = ((yy - 0.5 + half_span).ceil() as i32 - 1).min(highest_y);
                if y_low <= y_high {
                    let index = (row_offset + z) as usize;
                    column_min_y[index] = column_min_y[index].min(y_low);
                    column_max_y[index] = column_max_y[index].max(y_high);
                }
            }
        }
    }

    for grid_x in touched_min_x..=touched_max_x {
        let x = x_start + grid_x;
        for grid_z in touched_min_z..=touched_max_z {
            let index = (grid_x * size_xz + grid_z) as usize;
            let y_min = column_min_y[index];
            if y_min == i32::MAX {
                continue;
            }
            let z = z_start + grid_z;
            for y in y_min..=column_max_y[index] {
                let pos = BlockPos::new(x, y, z);
                let current = volume.get(pos);
                for target in &cfg.targets {
                    if can_place_ore(cfg, volume, rng, target, current, pos) {
                        volume.set(pos, target.state);
                        placed += 1;
                        break;
                    }
                }
            }
        }
    }

    placed > 0
}

pub(crate) fn can_place_ore<W: WorldGenVolume>(
    cfg: &CompiledOre,
    volume: &W,
    rng: &mut WorldgenRandom,
    target: &OreReplacement,
    state: VoxelId,
    pos: BlockPos,
) -> bool {
    if !target.target.test(state, pos.y, rng) {
        return false;
    }
    if should_skip_air_check(rng, cfg.discard_chance_on_air_exposure) {
        return true;
    }
    !is_adjacent_to_air(volume, pos)
}

fn should_skip_air_check(rng: &mut WorldgenRandom, discard_chance_on_air_exposure: f32) -> bool {
    if discard_chance_on_air_exposure <= 0.0 {
        true
    } else if discard_chance_on_air_exposure >= 1.0 {
        false
    } else {
        rng.next_f32() >= discard_chance_on_air_exposure
    }
}

fn is_adjacent_to_air<W: WorldGenVolume>(volume: &W, pos: BlockPos) -> bool {
    Direction::all()
        .into_iter()
        .any(|direction| volume.is_air(pos + direction.normal()))
}
