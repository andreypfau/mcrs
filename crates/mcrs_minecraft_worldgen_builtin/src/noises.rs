use mcrs_minecraft_noise::proto::NoiseParam;
use mcrs_minecraft_resource::{ResourceLocationBuf, StaticResourceLocation, rl};
use std::collections::BTreeMap;

#[rustfmt::skip]
const NOISES: &[(StaticResourceLocation, i32, &[f64])] = &[
    (rl!("minecraft:temperature"), -10, &[1.5, 0.0, 1.0, 0.0, 0.0, 0.0]),
    (rl!("minecraft:vegetation"), -8, &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
    (rl!("minecraft:continentalness"), -9, &[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:erosion"), -9, &[1.0, 1.0, 0.0, 1.0, 1.0]),
    (rl!("minecraft:temperature_large"), -12, &[1.5, 0.0, 1.0, 0.0, 0.0, 0.0]),
    (rl!("minecraft:vegetation_large"), -10, &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
    (rl!("minecraft:continentalness_large"), -11, &[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:erosion_large"), -11, &[1.0, 1.0, 0.0, 1.0, 1.0]),
    (rl!("minecraft:nether/temperature"), -7, &[1.0, 1.0]),
    (rl!("minecraft:nether/vegetation"), -7, &[1.0, 1.0]),
    (rl!("minecraft:ridge"), -7, &[1.0, 2.0, 1.0, 0.0, 0.0, 0.0]),
    (rl!("minecraft:offset"), -3, &[1.0, 1.0, 1.0, 0.0]),
    (rl!("minecraft:aquifer_barrier"), -3, &[1.0]),
    (rl!("minecraft:aquifer_fluid_level_floodedness"), -7, &[1.0]),
    (rl!("minecraft:aquifer_lava"), -1, &[1.0]),
    (rl!("minecraft:aquifer_fluid_level_spread"), -5, &[1.0]),
    (rl!("minecraft:pillar"), -7, &[1.0, 1.0]),
    (rl!("minecraft:pillar_rareness"), -8, &[1.0]),
    (rl!("minecraft:pillar_thickness"), -8, &[1.0]),
    (rl!("minecraft:spaghetti_2d"), -7, &[1.0]),
    (rl!("minecraft:spaghetti_2d_elevation"), -8, &[1.0]),
    (rl!("minecraft:spaghetti_2d_modulator"), -11, &[1.0]),
    (rl!("minecraft:spaghetti_2d_thickness"), -11, &[1.0]),
    (rl!("minecraft:spaghetti_3d_1"), -7, &[1.0]),
    (rl!("minecraft:spaghetti_3d_2"), -7, &[1.0]),
    (rl!("minecraft:spaghetti_3d_rarity"), -11, &[1.0]),
    (rl!("minecraft:spaghetti_3d_thickness"), -8, &[1.0]),
    (rl!("minecraft:spaghetti_roughness"), -5, &[1.0]),
    (rl!("minecraft:spaghetti_roughness_modulator"), -8, &[1.0]),
    (rl!("minecraft:cave_entrance"), -7, &[0.4, 0.5, 1.0]),
    (rl!("minecraft:cave_layer"), -8, &[1.0]),
    (rl!("minecraft:cave_cheese"), -8, &[0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 0.0, 2.0, 0.0]),
    (rl!("minecraft:ore_veininess"), -8, &[1.0]),
    (rl!("minecraft:ore_vein_a"), -7, &[1.0]),
    (rl!("minecraft:ore_vein_b"), -7, &[1.0]),
    (rl!("minecraft:ore_gap"), -5, &[1.0]),
    (rl!("minecraft:noodle"), -8, &[1.0]),
    (rl!("minecraft:noodle_thickness"), -8, &[1.0]),
    (rl!("minecraft:noodle_ridge_a"), -7, &[1.0]),
    (rl!("minecraft:noodle_ridge_b"), -7, &[1.0]),
    (rl!("minecraft:jagged"), -16, &[1.0; 16]),
    (rl!("minecraft:surface"), -6, &[1.0, 1.0, 1.0]),
    (rl!("minecraft:surface_secondary"), -6, &[1.0, 1.0, 0.0, 1.0]),
    (rl!("minecraft:clay_bands_offset"), -8, &[1.0]),
    (rl!("minecraft:badlands_pillar"), -2, &[1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:badlands_pillar_roof"), -8, &[1.0]),
    (rl!("minecraft:badlands_surface"), -6, &[1.0, 1.0, 1.0]),
    (rl!("minecraft:iceberg_pillar"), -6, &[1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:iceberg_pillar_roof"), -3, &[1.0]),
    (rl!("minecraft:iceberg_surface"), -6, &[1.0, 1.0, 1.0]),
    (rl!("minecraft:sulfur_cave_gradient"), -5, &[1.0, 0.0, 1.0]),
    (rl!("minecraft:ice_cave_gradient"), -4, &[1.0, 0.0, 1.0]),
    (rl!("minecraft:surface_swamp"), -2, &[1.0]),
    (rl!("minecraft:calcite"), -9, &[1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:gravel"), -8, &[1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:powder_snow"), -6, &[1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:packed_ice"), -7, &[1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:ice"), -4, &[1.0, 1.0, 1.0, 1.0]),
    (rl!("minecraft:soul_sand_layer"), -8, &[1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.013333333333333334]),
    (rl!("minecraft:gravel_layer"), -8, &[1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.013333333333333334]),
    (rl!("minecraft:patch"), -5, &[1.0, 0.0, 0.0, 0.0, 0.0, 0.013333333333333334]),
    (rl!("minecraft:small_patch"), -3, &[3.0]),
    (rl!("minecraft:netherrack"), -3, &[1.0, 0.0, 0.0, 0.35]),
    (rl!("minecraft:nether_wart"), -3, &[1.0, 0.0, 0.0, 0.9]),
    (rl!("minecraft:nether_state_selector"), -4, &[1.0]),
];

pub fn noises() -> BTreeMap<ResourceLocationBuf, NoiseParam> {
    NOISES
        .iter()
        .map(|(name, octave, amplitudes)| {
            let noise = NoiseParam::parity(*octave, amplitudes);
            ((*name).into(), noise)
        })
        .collect()
}
