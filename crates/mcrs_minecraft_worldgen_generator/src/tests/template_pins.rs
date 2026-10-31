use super::corpus_features;
use super::template_parity::{block_entity_types, feature_program, region};
use crate::feature_program::RunScratch;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_resource::rl;
use mcrs_voxel::BlockPos;

mod exhaustive {
    use super::*;

    /// A region whose bottom sits just under the floor clamps the fossil to ten
    /// blocks above that bottom, into the air, where every corner is empty; the
    /// three draws before the corner check still happen.
    #[test]
    fn a_fossil_with_too_many_empty_corners_places_nothing() {
        let node = &corpus_features().features[&rl!("minecraft:fossil_coal").to_arc()];
        let program = feature_program(node);
        let generator = program.generator_at(0, 0).expect("a fossil compiles");
        let mut region = region(BlockPos::new(-16, 60, -16), BlockPos::new(31, 99, 31), 0);
        let mut run = program.run(RunScratch::default());
        let mut rng = WorldgenRandom::new(7);
        let placed = generator.place(
            &mut run,
            &mut region,
            &mut rng,
            BlockPos::new(8, 64, 8),
            &|_| true,
        );
        assert!(!placed);
        assert!(region.writes.is_empty());
        let mut expected = WorldgenRandom::new(7);
        for bound in [4, 8, 10] {
            expected.next_i32_bound(bound);
        }
        assert_eq!(rng.next_i64(), expected.next_i64());
    }

    #[test]
    fn suspicious_sand_and_gravel_hold_a_brushable_block_entity() {
        let type_of = block_entity_types();
        for sand in ["minecraft:suspicious_sand", "minecraft:suspicious_gravel"] {
            assert_eq!(type_of[sand], "minecraft:brushable_block");
        }
    }
}
