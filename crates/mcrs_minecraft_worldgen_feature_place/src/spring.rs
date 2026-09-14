use crate::holds;
use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};

/// `west`, `east`, `north`, `south`, `below` — the order both counts walk.
const SIDES: [IVec3; 5] = [
    IVec3::new(-1, 0, 0),
    IVec3::new(1, 0, 0),
    IVec3::new(0, 0, -1),
    IVec3::new(0, 0, 1),
    IVec3::new(0, -1, 0),
];

#[derive(Clone, Debug)]
pub struct CompiledSpring {
    pub state: VoxelId,
    pub requires_block_below: bool,
    pub rock_count: i32,
    pub hole_count: i32,
    pub valid_blocks: StateMask,
}

/// `SpringFeature.place`, which draws nothing at all: every gate is a block
/// read.
pub fn place_spring<W: WorldGenVolume>(
    config: &CompiledSpring,
    volume: &mut W,
    _rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    if !volume.holds(&config.valid_blocks, at + IVec3::Y) {
        return false;
    }
    if config.requires_block_below && !volume.holds(&config.valid_blocks, at + IVec3::NEG_Y) {
        return false;
    }
    let current = volume.get(at);
    if !holds(&volume.world().air_states, current) && !holds(&config.valid_blocks, current) {
        return false;
    }

    let count = |volume: &W, mask: &StateMask| {
        SIDES
            .iter()
            .filter(|side| volume.holds(mask, at + **side))
            .count() as i32
    };
    if count(volume, &config.valid_blocks) != config.rock_count
        || count(volume, &volume.world().air_states) != config.hole_count
    {
        return false;
    }
    volume.set(at, config.state);
    true
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_chunk::Blocks;
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const LAVA: VoxelId = VoxelId(2);
    const DIRT: VoxelId = VoxelId(3);
    const ORIGIN: BlockPos = BlockPos::new(0, 40, 0);

    fn config(rock_count: i32, hole_count: i32) -> CompiledSpring {
        CompiledSpring {
            state: LAVA,
            requires_block_below: true,
            rock_count,
            hole_count,
            valid_blocks: mask_of([STONE]),
        }
    }

    /// Stone all round the origin, which is air.
    fn encased() -> FakeVolume {
        let mut volume = FakeVolume::default();
        for side in SIDES {
            let pos = ORIGIN + side;
            volume.blocks.insert((pos.x, pos.y, pos.z), STONE);
        }
        let above = ORIGIN + IVec3::Y;
        volume.blocks.insert((above.x, above.y, above.z), STONE);
        volume
    }

    #[test]
    fn places_when_both_counts_match() {
        let mut volume = encased();
        assert!(place_spring(
            &config(5, 0),
            &mut volume,
            &mut XoroshiroRandom::new(1),
            ORIGIN
        ));
        assert_eq!(volume.get(ORIGIN), LAVA);
    }

    #[test]
    fn one_hole_is_one_fewer_rock() {
        let mut volume = encased();
        let west = ORIGIN + SIDES[0];
        volume.blocks.remove(&(west.x, west.y, west.z));
        assert!(!place_spring(
            &config(5, 0),
            &mut volume,
            &mut XoroshiroRandom::new(1),
            ORIGIN
        ));
        assert!(place_spring(
            &config(4, 1),
            &mut volume,
            &mut XoroshiroRandom::new(1),
            ORIGIN
        ));
    }

    /// A hole is air, and a side that is neither valid nor air counts for
    /// neither total.
    #[test]
    fn a_foreign_side_counts_for_neither() {
        let mut volume = encased();
        let east = ORIGIN + SIDES[1];
        volume.blocks.insert((east.x, east.y, east.z), DIRT);
        assert!(place_spring(
            &config(4, 0),
            &mut volume,
            &mut XoroshiroRandom::new(1),
            ORIGIN
        ));
    }

    #[test]
    fn requires_block_below_gates_before_the_counts() {
        let mut volume = encased();
        let below = ORIGIN + IVec3::NEG_Y;
        volume.blocks.remove(&(below.x, below.y, below.z));
        assert!(!place_spring(
            &config(4, 1),
            &mut volume,
            &mut XoroshiroRandom::new(1),
            ORIGIN
        ));
        let mut open = config(4, 1);
        open.requires_block_below = false;
        assert!(place_spring(
            &open,
            &mut volume,
            &mut XoroshiroRandom::new(1),
            ORIGIN
        ));
    }

    #[test]
    fn draws_nothing() {
        let mut volume = encased();
        let mut rng = XoroshiroRandom::new(0x5eed);
        let before = rng.clone();
        place_spring(&config(5, 0), &mut volume, &mut rng, ORIGIN);
        assert_eq!(rng, before, "a spring spends no draw");
    }
}
