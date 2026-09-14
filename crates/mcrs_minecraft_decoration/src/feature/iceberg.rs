use crate::feature::holds;
use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;

#[derive(Clone, Debug)]
pub struct CompiledIceberg {
    pub state: VoxelId,
    pub ice_mask: StateMask,
    pub snow_block: VoxelId,
    pub snow_block_mask: StateMask,
    /// `snow`, the layer, which the smoothing pass drops when the block under
    /// it goes.
    pub snow_layer_mask: StateMask,
    /// `packed_ice`, `snow_block` and `blue_ice` — what the smoothing and
    /// carving passes recognise as iceberg, whichever state the feature writes.
    pub iceberg_mask: StateMask,
}

/// One iceberg, grown from the sea surface: a stack of noisy discs above the
/// water and a steeper stack below, smoothed, then usually hollowed by a
/// slanted ellipse.
///
/// Every radius is drawn before the cell it would shape is tested, so a disc
/// entirely outside the shape still costs its draws.
/// The shape one iceberg drew before any cell of it was tested.
struct Iceberg {
    origin: BlockPos,
    snow_on_top: bool,
    shape_angle: f64,
    ellipse_a: i32,
    ellipse_c: i32,
    is_ellipse: bool,
    over_water: i32,
    width: i32,
}

pub fn place_iceberg<W: WorldGenVolume>(
    cfg: &CompiledIceberg,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let origin = BlockPos::new(at.x, volume.extent().sea_level, at.z);
    let snow_on_top = rng.next_f64() > 0.7;
    let shape_angle = rng.next_f64() * 2.0 * std::f64::consts::PI;
    let ellipse_a = 11 - rng.next_i32_bound(5);
    let ellipse_c = 3 + rng.next_i32_bound(3);
    let is_ellipse = rng.next_f64() > 0.7;
    let mut over_water = if is_ellipse {
        rng.next_i32_bound(6) + 6
    } else {
        rng.next_i32_bound(15) + 3
    };
    if !is_ellipse && rng.next_f64() > 0.9 {
        over_water += rng.next_i32_bound(19) + 7;
    }
    let under_water = (over_water + rng.next_i32_bound(11)).min(18);
    let width = (over_water + rng.next_i32_bound(7) - rng.next_i32_bound(5)).min(11);
    let berg = Iceberg {
        origin,
        snow_on_top,
        shape_angle,
        ellipse_a,
        ellipse_c,
        is_ellipse,
        over_water,
        width,
    };
    let a = if is_ellipse { ellipse_a } else { 11 };

    for xo in -a..a {
        for zo in -a..a {
            for y_off in 0..over_water {
                let radius = if is_ellipse {
                    radius_ellipse(y_off, over_water, width)
                } else {
                    radius_round(rng, y_off, over_water, width)
                };
                if is_ellipse || xo < radius {
                    let local = IVec3::new(xo, y_off, zo);
                    generate_block(cfg, volume, rng, &berg, over_water, local, radius, a);
                }
            }
        }
    }

    smooth(cfg, volume, &berg);

    for xo in -a..a {
        for zo in -a..a {
            let mut y_off = -1;
            while y_off > -under_water {
                let layer_a = if is_ellipse {
                    (a as f32 * (1.0 - (y_off * y_off) as f32 / (under_water as f32 * 8.0))).ceil()
                        as i32
                } else {
                    a
                };
                let radius = radius_steep(rng, -y_off, under_water, width);
                if xo < radius {
                    let local = IVec3::new(xo, y_off, zo);
                    generate_block(cfg, volume, rng, &berg, under_water, local, radius, layer_a);
                }
                y_off -= 1;
            }
        }
    }

    let cut_out = if is_ellipse {
        rng.next_f64() > 0.1
    } else {
        rng.next_f64() > 0.7
    };
    if cut_out {
        generate_cut_out(cfg, volume, rng, &berg);
    }
    true
}

fn generate_cut_out<W: WorldGenVolume>(
    cfg: &CompiledIceberg,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    berg: &Iceberg,
) {
    let Iceberg {
        width,
        over_water: height,
        is_ellipse,
        ellipse_a,
        shape_angle,
        ..
    } = *berg;
    let sign_x = if rng.next_bool() { -1 } else { 1 };
    let sign_z = if rng.next_bool() { -1 } else { 1 };
    let near = (width - width / 2 - 1).max(1);
    let mut x_off = rng.next_i32_bound((width / 2 - 2).max(1));
    if rng.next_bool() {
        x_off = width / 2 + 1 - rng.next_i32_bound(near);
    }
    let mut z_off = rng.next_i32_bound((width / 2 - 2).max(1));
    if rng.next_bool() {
        z_off = width / 2 + 1 - rng.next_i32_bound(near);
    }
    if is_ellipse {
        x_off = rng.next_i32_bound((ellipse_a - 5).max(1));
        z_off = x_off;
    }

    let local = IVec3::new(sign_x * x_off, 0, sign_z * z_off);
    let angle = if is_ellipse {
        shape_angle + std::f64::consts::FRAC_PI_2
    } else {
        rng.next_f64() * 2.0 * std::f64::consts::PI
    };

    for y_off in 0..height - 3 {
        let radius = radius_round(rng, y_off, height, width);
        carve(cfg, volume, berg, radius, y_off, false, angle, local);
    }

    // The loop's own bound draws once per test, the failing one included.
    let mut y_off = -1;
    while y_off > -height + rng.next_i32_bound(5) {
        let radius = radius_steep(rng, -y_off, height, width);
        carve(cfg, volume, berg, radius, y_off, true, angle, local);
        y_off -= 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn carve<W: WorldGenVolume>(
    cfg: &CompiledIceberg,
    volume: &mut W,
    berg: &Iceberg,
    radius: i32,
    y_off: i32,
    under_water: bool,
    angle: f64,
    local: IVec3,
) {
    let origin = berg.origin;
    let a = radius + 1 + berg.ellipse_a / 3;
    let c = (radius - 3).min(3) + berg.ellipse_c / 2 - 1;
    for xo in -a..a {
        for zo in -a..a {
            if signed_distance_ellipse(xo, zo, local, a, c, angle) >= 0.0 {
                continue;
            }
            let pos = BlockPos::new(origin.x + xo, origin.y + y_off, origin.z + zo);
            let state = volume.get(pos);
            if !holds(&cfg.iceberg_mask, state) && !holds(&cfg.snow_block_mask, state) {
                continue;
            }
            if under_water {
                volume.set(pos, volume.world().water);
            } else {
                volume.set(pos, volume.world().air);
                let above = volume.get(pos + IVec3::Y);
                if holds(&cfg.snow_layer_mask, above) {
                    volume.set(pos + IVec3::Y, volume.world().air);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_block<W: WorldGenVolume>(
    cfg: &CompiledIceberg,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    berg: &Iceberg,
    height: i32,
    local: IVec3,
    radius: i32,
    a: i32,
) {
    let IVec3 {
        x: xo,
        y: y_off,
        z: zo,
    } = local;
    let is_ellipse = berg.is_ellipse;
    let distance = if is_ellipse {
        signed_distance_ellipse(
            xo,
            zo,
            IVec3::ZERO,
            a,
            ellipse_c_at(y_off, height, berg.ellipse_c),
            berg.shape_angle,
        )
    } else {
        signed_distance_circle(rng, xo, zo, radius)
    };
    if distance >= 0.0 {
        return;
    }
    let compare = if is_ellipse {
        -0.5
    } else {
        (-6 - rng.next_i32_bound(3)) as f64
    };
    if distance > compare && rng.next_f64() > 0.9 {
        return;
    }
    set_iceberg_block(
        cfg,
        volume,
        rng,
        berg,
        berg.origin + local,
        height - y_off,
        height,
    );
}

fn set_iceberg_block<W: WorldGenVolume>(
    cfg: &CompiledIceberg,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    berg: &Iceberg,
    pos: BlockPos,
    depth: i32,
    height: i32,
) {
    let (is_ellipse, snow_on_top) = (berg.is_ellipse, berg.snow_on_top);
    let state = volume.get(pos);
    let index = state.0 as usize;
    if !(volume.world().air_states.contains(index)
        || cfg.snow_block_mask.contains(index)
        || cfg.ice_mask.contains(index)
        || volume.world().water_states.contains(index))
    {
        return;
    }
    let randomness = !is_ellipse || rng.next_f64() > 0.05;
    let divisor = if is_ellipse { 3 } else { 2 };
    let water = volume.world().water_states.contains(index);
    if snow_on_top
        && !water
        && (depth as f64)
            <= rng.next_i32_bound((height / divisor).max(1)) as f64 + height as f64 * 0.6
        && randomness
    {
        volume.set(pos, cfg.snow_block);
    } else {
        volume.set(pos, cfg.state);
    }
}

fn smooth<W: WorldGenVolume>(cfg: &CompiledIceberg, volume: &mut W, berg: &Iceberg) {
    let (origin, height) = (berg.origin, berg.over_water);
    let a = if berg.is_ellipse {
        berg.ellipse_a
    } else {
        berg.width / 2
    };
    for x in -a..=a {
        for z in -a..=a {
            for y_off in 0..=height {
                let pos = BlockPos::new(origin.x + x, origin.y + y_off, origin.z + z);
                let state = volume.get(pos);
                let iceberg = holds(&cfg.iceberg_mask, state);
                if !iceberg && !holds(&cfg.snow_layer_mask, state) {
                    continue;
                }
                let below = volume.get(pos - IVec3::Y);
                if holds(&volume.world().air_states, below) {
                    volume.set(pos, volume.world().air);
                    volume.set(pos + IVec3::Y, volume.world().air);
                } else if iceberg {
                    let sides = [
                        volume.get(pos - IVec3::X),
                        volume.get(pos + IVec3::X),
                        volume.get(pos - IVec3::Z),
                        volume.get(pos + IVec3::Z),
                    ];
                    let exposed = sides
                        .iter()
                        .filter(|&&side| !holds(&cfg.iceberg_mask, side))
                        .count();
                    if exposed >= 3 {
                        volume.set(pos, volume.world().air);
                    }
                }
            }
        }
    }
}

fn ellipse_c_at(y_off: i32, height: i32, ellipse_c: i32) -> i32 {
    if y_off > 0 && height - y_off <= 3 {
        ellipse_c - (4 - (height - y_off))
    } else {
        ellipse_c
    }
}

fn signed_distance_circle(rng: &mut XoroshiroRandom, xo: i32, zo: i32, radius: i32) -> f64 {
    let offset = 10.0 * rng.next_f32().clamp(0.2, 0.8) / radius as f32;
    offset as f64 + (xo * xo) as f64 + (zo * zo) as f64 - (radius as f64).powi(2)
}

fn signed_distance_ellipse(xo: i32, zo: i32, origin: IVec3, a: i32, c: i32, angle: f64) -> f64 {
    let dx = (xo - origin.x) as f64;
    let dz = (zo - origin.z) as f64;
    ((dx * angle.cos() - dz * angle.sin()) / a as f64).powi(2)
        + ((dx * angle.sin() + dz * angle.cos()) / c as f64).powi(2)
        - 1.0
}

fn radius_round(rng: &mut XoroshiroRandom, y_off: i32, height: i32, width: i32) -> i32 {
    let k = 3.5 - rng.next_f32();
    let mut scale = (1.0 - (y_off * y_off) as f32 / (height as f32 * k)) * width as f32;
    if height > 15 + rng.next_i32_bound(5) {
        let capped = if y_off < 3 + rng.next_i32_bound(6) {
            y_off / 2
        } else {
            y_off
        };
        scale = (1.0 - capped as f32 / (height as f32 * k * 0.4)) * width as f32;
    }
    (scale / 2.0).ceil() as i32
}

fn radius_ellipse(y_off: i32, height: i32, width: i32) -> i32 {
    let scale = (1.0 - (y_off * y_off) as f32 / height as f32) * width as f32;
    (scale / 2.0).ceil() as i32
}

fn radius_steep(rng: &mut XoroshiroRandom, y_off: i32, height: i32, width: i32) -> i32 {
    let k = 1.0 + rng.next_f32() / 2.0;
    let scale = (1.0 - y_off as f32 / (height as f32 * k)) * width as f32;
    (scale / 2.0).ceil() as i32
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const PACKED_ICE: VoxelId = VoxelId(1);
    const BLUE_ICE: VoxelId = VoxelId(2);
    const SNOW_BLOCK: VoxelId = VoxelId(3);
    const SNOW_LAYER: VoxelId = VoxelId(4);
    const ICE: VoxelId = VoxelId(5);
    const WATER: VoxelId = VoxelId(6);
    const SEA_LEVEL: i32 = 63;
    const ORIGIN: BlockPos = BlockPos::new(8, 0, 8);

    fn config() -> CompiledIceberg {
        CompiledIceberg {
            state: PACKED_ICE,
            ice_mask: mask_of([ICE.0]),
            snow_block: SNOW_BLOCK,
            snow_block_mask: mask_of([SNOW_BLOCK.0]),
            snow_layer_mask: mask_of([SNOW_LAYER.0]),
            iceberg_mask: mask_of([PACKED_ICE.0, SNOW_BLOCK.0, BLUE_ICE.0]),
        }
    }

    /// Air over water to the sea surface, the shape an iceberg grows into.
    fn ocean() -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.water = WATER;
        volume.world.water_states = mask_of([WATER.0]);
        for x in ORIGIN.x - 20..=ORIGIN.x + 20 {
            for z in ORIGIN.z - 20..=ORIGIN.z + 20 {
                for y in SEA_LEVEL - 24..=SEA_LEVEL {
                    volume.blocks.insert((x, y, z), WATER);
                }
            }
        }
        volume
    }

    /// The prelude, in the reference's order. A test replays it to learn the
    /// shape the loops will then draw for.
    fn prelude(seed: u64) -> (bool, bool, i32, i32) {
        let mut rng = XoroshiroRandom::new(seed);
        let snow_on_top = rng.next_f64() > 0.7;
        rng.next_f64();
        rng.next_i32_bound(5);
        rng.next_i32_bound(3);
        let is_ellipse = rng.next_f64() > 0.7;
        let mut over_water = if is_ellipse {
            rng.next_i32_bound(6) + 6
        } else {
            rng.next_i32_bound(15) + 3
        };
        if !is_ellipse && rng.next_f64() > 0.9 {
            over_water += rng.next_i32_bound(19) + 7;
        }
        let under_water = (over_water + rng.next_i32_bound(11)).min(18);
        (snow_on_top, is_ellipse, over_water, under_water)
    }

    /// The eleven prelude draws decide the whole silhouette, so an iceberg's
    /// vertical extent is the one thing they can be read back from: nothing is
    /// written above the drawn over-water height or below the under-water one.
    #[test]
    fn the_silhouette_matches_the_prelude_draws() {
        let cfg = config();
        for seed in 0..24u64 {
            let (_, _, over_water, under_water) = prelude(seed);
            let mut volume = ocean();
            let mut rng = XoroshiroRandom::new(seed);
            assert!(place_iceberg(&cfg, &mut volume, &mut rng, ORIGIN));
            for ((x, y, z), _) in &volume.writes {
                assert!(
                    *y <= SEA_LEVEL + over_water && *y > SEA_LEVEL - under_water,
                    "seed {seed} wrote at y {y}, outside {} .. {}",
                    SEA_LEVEL - under_water,
                    SEA_LEVEL + over_water
                );
                assert!(
                    (x - ORIGIN.x).abs() <= 16 && (z - ORIGIN.z).abs() <= 16,
                    "seed {seed} wrote at {x},{z}, outside the volume"
                );
            }
            assert!(
                volume.writes.iter().any(|(_, state)| *state == PACKED_ICE),
                "seed {seed} produced no ice at all"
            );
        }
    }

    /// The round path draws nothing inside the block writer, so its whole draw
    /// count is geometry: this pins that count for one seed, recorded from this
    /// implementation rather than measured against the reference.
    #[test]
    fn a_round_iceberg_leaves_a_pinned_source_state() {
        let cfg = config();
        let seed = (0..1000u64)
            .find(|seed| {
                let (snow_on_top, is_ellipse, _, _) = prelude(*seed);
                !snow_on_top && !is_ellipse
            })
            .expect("a round iceberg without snow");
        let mut volume = ocean();
        let mut rng = XoroshiroRandom::new(seed);
        place_iceberg(&cfg, &mut volume, &mut rng, ORIGIN);
        assert_eq!(
            (seed, rng.next_i64()),
            (PINNED_ROUND_SEED, PINNED_ROUND_TAIL)
        );
    }

    const PINNED_ROUND_SEED: u64 = 0;
    const PINNED_ROUND_TAIL: i64 = 7_294_196_349_415_726_598;
}
