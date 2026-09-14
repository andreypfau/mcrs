use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};

/// The four stalk states are the reference's constants over `bamboo`, resolved
/// once: `age=1, stage=0, leaves=none` for the shaft, then the three tips.
#[derive(Clone, Debug)]
pub struct CompiledBamboo {
    pub probability: f32,
    pub supports_bamboo: StateMask,
    pub beneath_podzol_replaceable: StateMask,
    pub podzol: VoxelId,
    pub trunk: VoxelId,
    pub final_large: VoxelId,
    pub top_large: VoxelId,
    pub top_small: VoxelId,
}

/// A stalk of bamboo, with a disc of podzol under it at the given probability.
///
/// Reports success for any air origin, ground or no ground, and the ground test
/// gates every draw: an origin that is air over the wrong block spends nothing.
pub fn place_bamboo<W: WorldGenVolume>(
    cfg: &CompiledBamboo,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    if !volume.is_air(at) {
        return false;
    }
    if !volume.holds(&cfg.supports_bamboo, at - IVec3::Y) {
        return true;
    }

    let height = rng.next_i32_bound(12) + 5;
    if rng.next_f32() < cfg.probability {
        let radius = rng.next_i32_bound(4) + 1;
        for x in at.x - radius..=at.x + radius {
            for z in at.z - radius..=at.z + radius {
                let dx = x - at.x;
                let dz = z - at.z;
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let y = volume.height(HeightmapName::WorldSurface, x, z) - 1;
                if volume.holds(&cfg.beneath_podzol_replaceable, BlockPos::new(x, y, z)) {
                    volume.set(BlockPos::new(x, y, z), cfg.podzol);
                }
            }
        }
    }

    let mut y = at.y;
    for _ in 0..height {
        if !volume.is_air(BlockPos::new(at.x, y, at.z)) {
            break;
        }
        volume.set(BlockPos::new(at.x, y, at.z), cfg.trunk);
        y += 1;
    }

    // The cap overwrites whatever stopped the shaft, air or not.
    if y - at.y >= 3 {
        volume.set(BlockPos::new(at.x, y, at.z), cfg.final_large);
        volume.set(BlockPos::new(at.x, y - 1, at.z), cfg.top_large);
        volume.set(BlockPos::new(at.x, y - 2, at.z), cfg.top_small);
    }
    true
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_chunk::Blocks;
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const DIRT: VoxelId = VoxelId(1);
    const PODZOL: VoxelId = VoxelId(2);
    const TRUNK: VoxelId = VoxelId(3);
    const FINAL_LARGE: VoxelId = VoxelId(4);
    const TOP_LARGE: VoxelId = VoxelId(5);
    const TOP_SMALL: VoxelId = VoxelId(6);
    const STONE: VoxelId = VoxelId(7);
    const ORIGIN: BlockPos = BlockPos::new(8, 64, 8);

    fn config(probability: f32) -> CompiledBamboo {
        CompiledBamboo {
            probability,
            supports_bamboo: mask_of([DIRT]),
            beneath_podzol_replaceable: mask_of([DIRT]),
            podzol: PODZOL,
            trunk: TRUNK,
            final_large: FINAL_LARGE,
            top_large: TOP_LARGE,
            top_small: TOP_SMALL,
        }
    }

    fn ground() -> FakeVolume {
        let mut volume = FakeVolume::default();
        for x in ORIGIN.x - 8..=ORIGIN.x + 8 {
            for z in ORIGIN.z - 8..=ORIGIN.z + 8 {
                volume.blocks.insert((x, ORIGIN.y - 1, z), DIRT);
                volume.heights.insert((x, z), ORIGIN.y);
            }
        }
        volume
    }

    /// The shaft runs to its drawn height and the three tip states sit on top,
    /// and the draws are exactly the height, the podzol roll and — only when it
    /// hits — the disc radius.
    #[test]
    fn a_stalk_grows_to_its_height_and_the_disc_costs_one_more_draw() {
        let cfg = config(1.0);
        let mut volume = ground();
        let mut rng = XoroshiroRandom::new(0x2b00_b1e5);
        assert!(place_bamboo(&cfg, &mut volume, &mut rng, ORIGIN));

        let mut replay = XoroshiroRandom::new(0x2b00_b1e5);
        let height = replay.next_i32_bound(12) + 5;
        assert!(replay.next_f32() < 1.0);
        replay.next_i32_bound(4);
        assert_eq!(
            rng, replay,
            "height, podzol roll, disc radius — and no more"
        );

        let top = ORIGIN.y + height;
        assert_eq!(
            volume.get(BlockPos::new(ORIGIN.x, top, ORIGIN.z)),
            FINAL_LARGE
        );
        assert_eq!(
            volume.get(BlockPos::new(ORIGIN.x, top - 1, ORIGIN.z)),
            TOP_LARGE
        );
        assert_eq!(
            volume.get(BlockPos::new(ORIGIN.x, top - 2, ORIGIN.z)),
            TOP_SMALL
        );
        assert_eq!(volume.get(ORIGIN), TRUNK);
        assert_eq!(
            volume.get(ORIGIN - IVec3::Y),
            PODZOL,
            "the disc replaces the ground it stands on"
        );
    }

    /// A probability of zero costs the roll and nothing else, and leaves the
    /// ground alone.
    #[test]
    fn without_podzol_the_radius_is_never_drawn() {
        let cfg = config(0.0);
        let mut volume = ground();
        let mut rng = XoroshiroRandom::new(99);
        assert!(place_bamboo(&cfg, &mut volume, &mut rng, ORIGIN));

        let mut replay = XoroshiroRandom::new(99);
        replay.next_i32_bound(12);
        replay.next_f32();
        assert_eq!(rng, replay, "height and the roll, no radius");
        assert!(
            volume.writes.iter().all(|(_, state)| *state != PODZOL),
            "no podzol at probability zero"
        );
    }

    /// Air over the wrong ground still reports success — the reference counts
    /// the origin, not the stalk — but draws nothing at all.
    #[test]
    fn ground_that_refuses_bamboo_succeeds_and_draws_nothing() {
        let cfg = config(1.0);
        let mut volume = ground();
        volume
            .blocks
            .insert((ORIGIN.x, ORIGIN.y - 1, ORIGIN.z), STONE);
        let mut rng = XoroshiroRandom::new(7);
        let before = rng.clone();
        assert!(place_bamboo(&cfg, &mut volume, &mut rng, ORIGIN));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    /// A blocked origin is the one failure, and it too is free.
    #[test]
    fn a_blocked_origin_fails_and_draws_nothing() {
        let cfg = config(1.0);
        let mut volume = ground();
        volume.blocks.insert((ORIGIN.x, ORIGIN.y, ORIGIN.z), STONE);
        let mut rng = XoroshiroRandom::new(7);
        let before = rng.clone();
        assert!(!place_bamboo(&cfg, &mut volume, &mut rng, ORIGIN));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    /// A ceiling three blocks up leaves the shaft too short for the tip, so the
    /// three tip states are never written.
    #[test]
    fn a_shaft_under_three_blocks_gets_no_tip() {
        let cfg = config(0.0);
        let mut volume = ground();
        volume
            .blocks
            .insert((ORIGIN.x, ORIGIN.y + 2, ORIGIN.z), STONE);
        let mut rng = XoroshiroRandom::new(5150);
        assert!(place_bamboo(&cfg, &mut volume, &mut rng, ORIGIN));
        assert!(
            volume
                .writes
                .iter()
                .all(|(_, state)| *state == TRUNK || *state == PODZOL),
            "only shaft blocks: {:?}",
            volume.writes
        );
    }
}
