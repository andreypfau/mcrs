use crate::ore_vein_parity::{MIN_Y, stone_world};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placer::Rule;
use mcrs_minecraft_worldgen_feature_place::ore_modern::{
    CompiledOre, OreReplacement, OreScratch, place_modern_ore,
};

/// The probe box is the one thing the flat-world dump cannot pin, because the
/// oracle stands `getHeight` in for a constant. At size 20 from (0, 64, 0) the
/// reference's arithmetic gives a spread of 2.5 rounded up to 3 and a max radius
/// of 2, so columns -5..=4 are tested against y 60.
#[test]
fn the_probe_box_is_the_reference_box() {
    let cfg = CompiledOre {
        targets: vec![OreReplacement {
            target: Rule::AlwaysTrue,
            state: VoxelId(3),
        }],
        size: 20,
        discard_chance_on_air_exposure: 0.0,
    };
    let probe = |floor_at: Option<[i32; 2]>| {
        let mut world = stone_world([0, 64, 0], VoxelId(1), VoxelId(0), 59)
            .with_height(move |_, _, x, z| if floor_at == Some([x, z]) { 60 } else { 59 });
        place_modern_ore(
            &cfg,
            &mut world,
            &mut WorldgenRandom::new(42),
            BlockPos::new(0, 64, 0),
            &mut OreScratch::default(),
        )
    };
    assert!(!probe(None), "every column below y 60 must abort the vein");
    assert!(probe(Some([-5, -5])), "the box starts at -5");
    assert!(probe(Some([4, 4])), "the box ends at 4");
    assert!(!probe(Some([5, 5])), "the box does not reach 5");
    assert!(!probe(Some([-6, 0])), "the box does not reach -6");
    assert!(!probe(Some([0, 6])), "the box does not reach 6");
}

/// The three segment draws are spent before the probe, so a vein whose probe box
/// is entirely below the floor still advances the source.
#[test]
fn a_failed_probe_still_costs_the_segment_draws() {
    let cfg = CompiledOre {
        targets: vec![OreReplacement {
            target: Rule::AlwaysTrue,
            state: VoxelId(3),
        }],
        size: 16,
        discard_chance_on_air_exposure: 0.0,
    };
    let mut world = stone_world([0, 200, 0], VoxelId(1), VoxelId(0), MIN_Y);
    let mut rng = WorldgenRandom::new(42);
    assert!(!place_modern_ore(
        &cfg,
        &mut world,
        &mut rng,
        BlockPos::new(0, 200, 0),
        &mut OreScratch::default(),
    ));
    assert!(world.writes.is_empty());

    let mut replay = WorldgenRandom::new(42);
    replay.next_f32();
    replay.next_i32_bound(3);
    replay.next_i32_bound(3);
    assert_eq!(rng, replay, "an aborted vein must draw exactly three times");
}
