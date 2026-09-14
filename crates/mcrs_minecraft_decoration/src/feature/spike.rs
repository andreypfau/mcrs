use crate::feature::holds;
use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, WorldGenVolume};
use mcrs_voxel_storage::VoxelId;

#[derive(Clone, Debug)]
pub struct CompiledSpike {
    pub state: VoxelId,
    pub can_place_on: Predicate,
    pub can_replace: Predicate,
}

/// The reference's floor for the root that hangs under a spike, absolute and
/// independent of the dimension's own bottom.
const ROOT_FLOOR: i32 = 50;

/// An ice spike: a tapering cone of `state`, mirrored below its base when the
/// cone is wide enough, then up to nine roots bored downwards from it.
pub fn place_spike<W: WorldGenVolume>(
    cfg: &CompiledSpike,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let floor = volume.extent().min_y + 2;
    let mut origin = at;
    while volume.is_air(origin) && origin.y > floor {
        origin.y -= 1;
    }
    if !cfg.can_place_on.test(volume, origin) {
        return false;
    }

    origin.y += rng.next_i32_bound(4);
    let height = rng.next_i32_bound(4) + 7;
    let width = height / 4 + rng.next_i32_bound(2);
    if width > 1 && rng.next_i32_bound(60) == 0 {
        origin.y += 10 + rng.next_i32_bound(30);
    }

    for y_off in 0..height {
        let scale = (1.0 - y_off as f32 / height as f32) * width as f32;
        let edge = scale.ceil() as i32;
        for xo in -edge..=edge {
            let dx = xo.abs() as f32 - 0.25;
            for zo in -edge..=edge {
                let dz = zo.abs() as f32 - 0.25;
                let inside = (xo == 0 && zo == 0) || dx * dx + dz * dz <= scale * scale;
                // The rim draw is spent per rim cell of every disc, and only
                // for cells the ellipse already accepted.
                let kept = inside
                    && ((xo != -edge && xo != edge && zo != -edge && zo != edge)
                        || rng.next_f32() <= 0.75);
                if !kept {
                    continue;
                }
                replace(cfg, volume, origin + IVec3::new(xo, y_off, zo));
                if y_off != 0 && edge > 1 {
                    replace(
                        cfg,
                        volume,
                        BlockPos::new(origin.x + xo, origin.y - y_off, origin.z + zo),
                    );
                }
            }
        }
    }

    let root_width = (width - 1).clamp(0, 1);
    for xo in -root_width..=root_width {
        for zo in -root_width..=root_width {
            let mut cursor = BlockPos::new(origin.x + xo, origin.y - 1, origin.z + zo);
            let mut run = if xo.abs() == 1 && zo.abs() == 1 {
                rng.next_i32_bound(5)
            } else {
                50
            };
            while cursor.y > ROOT_FLOOR {
                let state = volume.get(cursor);
                if !holds(&volume.world().air_states, state)
                    && !cfg.can_replace.test(volume, cursor)
                    && state != cfg.state
                {
                    break;
                }
                volume.set(cursor, cfg.state);
                cursor.y -= 1;
                run -= 1;
                if run <= 0 {
                    cursor.y -= rng.next_i32_bound(5) + 1;
                    run = rng.next_i32_bound(5);
                }
            }
        }
    }
    true
}

fn replace<W: WorldGenVolume>(cfg: &CompiledSpike, volume: &mut W, pos: BlockPos) {
    if volume.is_air(pos) || cfg.can_replace.test(volume, pos) {
        volume.set(pos, cfg.state);
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const SNOW: VoxelId = VoxelId(1);
    const PACKED_ICE: VoxelId = VoxelId(2);
    const BEDROCK: VoxelId = VoxelId(3);
    const ORIGIN: BlockPos = BlockPos::new(8, 80, 8);

    fn config() -> CompiledSpike {
        CompiledSpike {
            state: PACKED_ICE,
            can_place_on: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([SNOW]),
            },
            can_replace: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([SNOW]),
            },
        }
    }

    /// Snow under a stack of air: the descent walks down to it without drawing.
    fn snowfield() -> FakeVolume {
        let mut volume = FakeVolume::default();
        for x in ORIGIN.x - 20..=ORIGIN.x + 20 {
            for z in ORIGIN.z - 20..=ORIGIN.z + 20 {
                volume.blocks.insert((x, 70, z), SNOW);
                for y in 40..70 {
                    volume.blocks.insert((x, y, z), BEDROCK);
                }
            }
        }
        volume
    }

    /// A seed whose spike stands on the snow with no lift and no sixtieth-chance
    /// jump, so its roots meet bedrock on their first read and the draw sequence
    /// is a closed form.
    fn grounded_seed() -> u64 {
        for seed in 0..10_000u64 {
            let mut rng = XoroshiroRandom::new(seed);
            if rng.next_i32_bound(4) != 0 {
                continue;
            }
            let height = rng.next_i32_bound(4) + 7;
            let width = height / 4 + rng.next_i32_bound(2);
            if width <= 1 || rng.next_i32_bound(60) == 0 {
                continue;
            }
            return seed;
        }
        panic!("no seed lands a spike flat on the snow");
    }

    /// The four shaping draws come first, the sixtieth-chance roll only once the
    /// cone is more than one wide, one rim draw per rim cell the ellipse kept,
    /// and one per diagonal root.
    #[test]
    fn a_spike_draws_its_shape_then_its_rim_then_its_roots() {
        let cfg = config();
        let seed = grounded_seed();
        let mut volume = snowfield();
        let mut rng = XoroshiroRandom::new(seed);
        assert!(place_spike(&cfg, &mut volume, &mut rng, ORIGIN));

        let mut replay = XoroshiroRandom::new(seed);
        assert_eq!(replay.next_i32_bound(4), 0);
        let height = replay.next_i32_bound(4) + 7;
        let width = height / 4 + replay.next_i32_bound(2);
        replay.next_i32_bound(60);

        let mut rim = 0;
        for y_off in 0..height {
            let scale = (1.0 - y_off as f32 / height as f32) * width as f32;
            let edge = scale.ceil() as i32;
            for xo in -edge..=edge {
                let dx = xo.abs() as f32 - 0.25;
                for zo in -edge..=edge {
                    let dz = zo.abs() as f32 - 0.25;
                    let inside = (xo == 0 && zo == 0) || dx * dx + dz * dz <= scale * scale;
                    if inside && (xo == -edge || xo == edge || zo == -edge || zo == edge) {
                        replay.next_f32();
                        rim += 1;
                    }
                }
            }
        }
        assert!(rim > 0, "the cone has a rim");

        let root_width = (width - 1).clamp(0, 1);
        for xo in -root_width..=root_width {
            for zo in -root_width..=root_width {
                if xo.abs() == 1 && zo.abs() == 1 {
                    replay.next_i32_bound(5);
                }
            }
        }
        assert_eq!(rng, replay, "shape, rim, then one draw per diagonal root");
        assert!(volume.writes.iter().all(|(_, state)| *state == PACKED_ICE));
    }

    /// The descent is free and the wrong ground stops the feature before the
    /// first shaping draw.
    #[test]
    fn ground_the_predicate_refuses_draws_nothing() {
        let cfg = config();
        let mut volume = FakeVolume::default();
        for x in ORIGIN.x - 4..=ORIGIN.x + 4 {
            for z in ORIGIN.z - 4..=ORIGIN.z + 4 {
                for y in 40..=70 {
                    volume.blocks.insert((x, y, z), BEDROCK);
                }
            }
        }
        let mut rng = XoroshiroRandom::new(11);
        let before = rng.clone();
        assert!(!place_spike(&cfg, &mut volume, &mut rng, ORIGIN));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    /// The cone stays inside the 3x3 volume: at most three cells of overhang
    /// either way, far short of the sixteen the writer allows.
    #[test]
    fn the_footprint_never_leaves_the_window() {
        let cfg = config();
        for seed in 0..48u64 {
            let mut volume = snowfield();
            let mut rng = XoroshiroRandom::new(seed);
            place_spike(&cfg, &mut volume, &mut rng, ORIGIN);
            for ((x, _, z), _) in &volume.writes {
                assert!(
                    (x - ORIGIN.x).abs() <= 16 && (z - ORIGIN.z).abs() <= 16,
                    "write at {x},{z} left the volume on seed {seed}"
                );
            }
        }
    }
}
