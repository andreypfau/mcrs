use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_voxel_math::BlockPos;

use crate::feature::ore_modern::{CompiledOre, can_place_ore};
use mcrs_minecraft_worldgen::feature::placer::WorldGenVolume;
const MAX_DIST_FROM_ORIGIN: i32 = 7;

/// `ScatteredOreFeature.place`: up to `size` single blocks, each drifting a
/// little further from the origin than the last. It shares its configuration
/// and its air-exposure test with the vein of [`crate::feature::ore_modern`];
/// only the spread differs.
///
/// It reports success whether or not anything was replaced, which is the
/// reference's unconditional `true`.
pub fn place_scattered_ore<W: WorldGenVolume>(
    config: &CompiledOre,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
) -> bool {
    let tries = rng.next_i32_bound(config.size + 1);
    for try_index in 0..tries {
        let spread = try_index.min(MAX_DIST_FROM_ORIGIN);
        let offset = IVec3::new(
            axis_offset(rng, spread),
            axis_offset(rng, spread),
            axis_offset(rng, spread),
        );
        let at = origin + offset;
        let state = volume.get(at);
        for target in &config.targets {
            if can_place_ore(config, volume, rng, target, state, at) {
                volume.set(at, target.state);
                break;
            }
        }
    }
    true
}

/// `Math.round(float)` is `floor(x + 0.5)` over the exact value, which a `f32`
/// addition does not always give; the widening does.
fn axis_offset(rng: &mut XoroshiroRandom, max: i32) -> i32 {
    let drift = (rng.next_f32() - rng.next_f32()) * max as f32;
    (drift as f64 + 0.5).floor() as i32
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_voxel_storage::VoxelId;

    use super::*;
    use crate::feature::ore_modern::OreReplacement;
    use mcrs_minecraft_worldgen::feature::placer::{BoxRegion, Rule, single_state};

    const STONE: VoxelId = VoxelId(1);
    const DEBRIS: VoxelId = VoxelId(2);
    const ORIGIN: BlockPos = BlockPos::new(0, 40, 0);
    const SEED: u64 = 0x5ca7_7e6d;

    fn rock() -> BoxRegion {
        BoxRegion::columns(2, -64, 319, STONE)
    }

    fn config(discard_chance_on_air_exposure: f32) -> CompiledOre {
        CompiledOre {
            targets: vec![OreReplacement {
                target: Rule::MatchingStates(single_state(STONE)),
                state: DEBRIS,
            }],
            size: 3,
            discard_chance_on_air_exposure,
        }
    }

    /// One bounded int for the try count, then two floats per axis per try, and
    /// nothing else while the discard chance is 1 — the air check is a read.
    #[test]
    fn every_try_costs_six_floats_and_lands_within_its_own_radius() {
        let config = config(1.0);
        let mut volume = rock();
        let mut rng = XoroshiroRandom::new(SEED);
        assert!(place_scattered_ore(&config, &mut volume, &mut rng, ORIGIN));

        let mut replay = XoroshiroRandom::new(SEED);
        let tries = replay.next_i32_bound(config.size + 1);
        for try_index in 0..tries {
            let spread = try_index.min(MAX_DIST_FROM_ORIGIN);
            for _ in 0..3 {
                let drift = (replay.next_f32() - replay.next_f32()) * spread as f32;
                assert!((drift as f64 + 0.5).floor().abs() as i32 <= spread);
            }
        }
        assert_eq!(rng, replay, "{tries} tries of six floats each");
        assert_eq!(volume.writes.len(), tries as usize);
        assert!(volume.writes.iter().all(|(_, state)| *state == DEBRIS));
    }

    /// The first try never drifts: its radius is zero, so all three draws round
    /// to the origin.
    #[test]
    fn the_first_try_sits_on_the_origin() {
        let config = config(1.0);
        let mut volume = rock();
        let mut rng = XoroshiroRandom::new(SEED);
        place_scattered_ore(&config, &mut volume, &mut rng, ORIGIN);
        assert_eq!(volume.writes.first().map(|(at, _)| *at), Some(ORIGIN));
    }

    /// A fractional discard chance draws one more float per candidate, and a
    /// candidate the rule rejects draws none.
    #[test]
    fn a_fractional_discard_chance_draws_once_per_candidate() {
        let config = config(0.5);
        let mut volume = rock();
        let mut rng = XoroshiroRandom::new(SEED);
        place_scattered_ore(&config, &mut volume, &mut rng, ORIGIN);

        let mut replay = XoroshiroRandom::new(SEED);
        let tries = replay.next_i32_bound(config.size + 1);
        for _ in 0..tries {
            for _ in 0..6 {
                replay.next_f32();
            }
            replay.next_f32();
        }
        assert_eq!(rng, replay);
    }

    /// A size of zero draws the try count and stops, and still reports success.
    #[test]
    fn a_zero_size_vein_draws_once_and_places_nothing() {
        let mut config = config(1.0);
        config.size = 0;
        let mut volume = rock();
        let mut rng = XoroshiroRandom::new(SEED);
        assert!(place_scattered_ore(&config, &mut volume, &mut rng, ORIGIN));

        let mut replay = XoroshiroRandom::new(SEED);
        replay.next_i32_bound(1);
        assert_eq!(rng, replay);
        assert!(volume.writes.is_empty());
    }
}
