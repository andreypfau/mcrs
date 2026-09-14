use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, WorldGenVolume};
use mcrs_minecraft_worldgen::value_provider::IntProvider;

use crate::feature::tree::provider::StateProvider;

#[derive(Clone, Debug)]
pub struct ColumnLayer {
    pub height: IntProvider,
    pub provider: StateProvider,
}

/// One `block_column` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledBlockColumn {
    pub layers: Vec<ColumnLayer>,
    pub direction: Direction,
    pub allowed_placement: Predicate,
    /// Which end of the column loses the cells a blocked run takes away: the
    /// tip when set, otherwise the base.
    pub prioritize_tip: bool,
}

/// `BlockColumnFeature.place`: every layer's height first, then the free-space
/// walk that may shorten them, then one provider draw per cell.
pub fn place_block_column<W: WorldGenVolume>(
    cfg: &CompiledBlockColumn,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let mut heights: Vec<i32> = cfg
        .layers
        .iter()
        .map(|layer| layer.height.sample(rng))
        .collect();
    let total: i32 = heights.iter().sum();
    if total == 0 {
        return false;
    }

    // The reference tests the cell one step past the origin first and never
    // tests the origin itself, so a column always writes its own first cell.
    let mut probe = at + cfg.direction.normal();
    for reached in 0..total {
        if !cfg.allowed_placement.test(volume, probe) {
            truncate(&mut heights, total, reached, cfg.prioritize_tip);
            break;
        }
        probe += cfg.direction.normal();
    }

    let mut place = at;
    for (layer, count) in cfg.layers.iter().zip(&heights) {
        for _ in 0..*count {
            let state = layer.provider.state(volume, rng, place);
            volume.set(place, state);
            place += cfg.direction.normal();
        }
    }
    true
}

fn truncate(heights: &mut [i32], total: i32, new_height: i32, prioritize_tip: bool) {
    let mut to_remove = total - new_height;
    for step in 0..heights.len() {
        if to_remove <= 0 {
            break;
        }
        let index = if prioritize_tip {
            step
        } else {
            heights.len() - 1 - step
        };
        let taken = heights[index].min(to_remove);
        to_remove -= taken;
        heights[index] -= taken;
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::IVec3;
    use mcrs_minecraft_random::Random;

    use mcrs_minecraft_chunk::VoxelId;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::feature::placer::single_state;
    use mcrs_minecraft_worldgen::value_provider::DispatchedIntProvider;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const STEM: VoxelId = VoxelId(3);
    const TIP: VoxelId = VoxelId(4);
    const STONE: VoxelId = VoxelId(5);
    const AT: BlockPos = BlockPos::new(0, 64, 0);

    fn config(prioritize_tip: bool) -> CompiledBlockColumn {
        CompiledBlockColumn {
            layers: vec![
                ColumnLayer {
                    height: IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                        min_inclusive: 2,
                        max_inclusive: 4,
                    }),
                    provider: StateProvider::Simple(STEM),
                },
                ColumnLayer {
                    height: IntProvider::Constant(1),
                    provider: StateProvider::Simple(TIP),
                },
            ],
            direction: Direction::Up,
            allowed_placement: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: single_state(AIR),
            },
            prioritize_tip,
        }
    }

    /// The heights are drawn in layer order, before anything is read or
    /// written, and the providers draw once per cell afterwards.
    #[test]
    fn the_layer_heights_are_drawn_first_and_the_column_writes_bottom_up() {
        let cfg = config(true);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(9);

        assert!(place_block_column(&cfg, &mut volume, &mut rng, AT));

        let mut replay = XoroshiroRandom::new(9);
        let stem = replay.next_int_between_inclusive(2, 4);
        assert_eq!(
            rng, replay,
            "two heights, and simple providers draw nothing"
        );

        let mut expected: Vec<((i32, i32, i32), VoxelId)> = (0..stem)
            .map(|step| ((AT.x, AT.y + step, AT.z), STEM))
            .collect();
        expected.push(((AT.x, AT.y + stem, AT.z), TIP));
        assert_eq!(volume.writes, expected);
    }

    /// A blocked cell shortens the column, and `prioritize_tip` decides which
    /// layer gives the cells up: the first layer when the tip has priority,
    /// the last layer when it does not.
    #[test]
    fn a_blocked_run_takes_its_cells_from_the_base_or_from_the_tip() {
        for (prioritize_tip, expected) in [(true, vec![TIP]), (false, vec![STEM])] {
            let mut cfg = config(prioritize_tip);
            cfg.layers[0].height = IntProvider::Constant(3);
            let mut volume = FakeVolume::with([((AT.x, AT.y + 2, AT.z), STONE)]);
            let mut rng = XoroshiroRandom::new(3);

            assert!(place_block_column(&cfg, &mut volume, &mut rng, AT));
            let written: Vec<VoxelId> = volume.writes.iter().map(|(_, state)| *state).collect();
            assert_eq!(written, expected, "prioritize_tip = {prioritize_tip}");
        }
    }

    /// Zero total height is the one refusal, and the heights are already drawn
    /// by then.
    #[test]
    fn a_column_of_no_height_places_nothing() {
        let mut cfg = config(false);
        cfg.layers = vec![ColumnLayer {
            height: IntProvider::Constant(0),
            provider: StateProvider::Simple(STEM),
        }];
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(1);

        assert!(!place_block_column(&cfg, &mut volume, &mut rng, AT));
        assert!(volume.writes.is_empty());
    }

    /// Downward columns walk the other way, tip first.
    #[test]
    fn a_downward_column_grows_below_the_origin() {
        let mut cfg = config(true);
        cfg.direction = Direction::Down;
        cfg.layers[0].height = IntProvider::Constant(2);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(2);

        assert!(place_block_column(&cfg, &mut volume, &mut rng, AT));
        assert_eq!(
            volume.writes,
            vec![
                ((AT.x, AT.y, AT.z), STEM),
                ((AT.x, AT.y - 1, AT.z), STEM),
                ((AT.x, AT.y - 2, AT.z), TIP),
            ]
        );
    }
}
