use bevy_math::IVec3;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{Rule, WorldGenVolume};
use mcrs_voxel_storage::VoxelId;

#[derive(Clone, Debug)]
pub struct Replacement {
    pub target: Rule,
    pub state: VoxelId,
}

#[derive(Clone, Debug)]
pub struct CompiledReplaceSingleBlock {
    pub targets: Vec<Replacement>,
}

/// `ReplaceBlockFeature.place`: the first entry whose rule matches the block at
/// the origin writes, and the rest are not tested. It always reports success,
/// so a sequence holding one does not end on it.
pub fn place_replace_single_block<W: WorldGenVolume>(
    cfg: &CompiledReplaceSingleBlock,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: IVec3,
) -> bool {
    let here = volume.get(at);
    for replacement in &cfg.targets {
        if replacement.target.test(here, at.y, rng) {
            volume.set(at, replacement.state);
            break;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::feature::placer::{BoxRegion, WorldStates, mask_of};
    use mcrs_voxel_storage::Blocks;

    const STONE: VoxelId = VoxelId(1);
    const DEEPSLATE: VoxelId = VoxelId(2);
    const GOLD: VoxelId = VoxelId(3);
    const IRON: VoxelId = VoxelId(4);

    fn volume(fill: VoxelId) -> BoxRegion {
        let mut volume = BoxRegion::new(IVec3::ZERO, IVec3::new(3, 3, 3), fill);
        volume.world = WorldStates::default();
        volume
    }

    fn config() -> CompiledReplaceSingleBlock {
        CompiledReplaceSingleBlock {
            targets: vec![
                Replacement {
                    target: Rule::MatchingStates(mask_of([STONE])),
                    state: GOLD,
                },
                Replacement {
                    target: Rule::MatchingStates(mask_of([DEEPSLATE])),
                    state: IRON,
                },
            ],
        }
    }

    #[test]
    fn the_first_matching_rule_writes_and_the_rest_are_not_tested() {
        let mut volume = volume(STONE);
        let mut rng = XoroshiroRandom::new(1);
        assert!(place_replace_single_block(
            &config(),
            &mut volume,
            &mut rng,
            IVec3::new(1, 1, 1)
        ));
        assert_eq!(volume.get(IVec3::new(1, 1, 1)), GOLD);
    }

    #[test]
    fn a_later_entry_still_matches_when_the_first_does_not() {
        let mut volume = volume(DEEPSLATE);
        let mut rng = XoroshiroRandom::new(1);
        place_replace_single_block(&config(), &mut volume, &mut rng, IVec3::new(1, 1, 1));
        assert_eq!(volume.get(IVec3::new(1, 1, 1)), IRON);
    }

    #[test]
    fn no_match_writes_nothing_and_still_reports_success() {
        let mut volume = volume(VoxelId(9));
        let mut rng = XoroshiroRandom::new(1);
        assert!(place_replace_single_block(
            &config(),
            &mut volume,
            &mut rng,
            IVec3::new(1, 1, 1)
        ));
        assert!(volume.writes.is_empty());
    }
}
