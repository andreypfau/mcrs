use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use std::collections::BTreeMap;

#[rustfmt::skip]
const NOISES: &[(&str, i32, &[f64])] = &[
    ("temperature", -10, &[1.5, 0.0, 1.0, 0.0, 0.0, 0.0]),
    ("vegetation", -8, &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
    ("continentalness", -9, &[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0]),
    ("erosion", -9, &[1.0, 1.0, 0.0, 1.0, 1.0]),
    ("temperature_large", -12, &[1.5, 0.0, 1.0, 0.0, 0.0, 0.0]),
    ("vegetation_large", -10, &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
    ("continentalness_large", -11, &[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0]),
    ("erosion_large", -11, &[1.0, 1.0, 0.0, 1.0, 1.0]),
    ("nether/temperature", -7, &[1.0, 1.0]),
    ("nether/vegetation", -7, &[1.0, 1.0]),
    ("ridge", -7, &[1.0, 2.0, 1.0, 0.0, 0.0, 0.0]),
    ("offset", -3, &[1.0, 1.0, 1.0, 0.0]),
    ("aquifer_barrier", -3, &[1.0]),
    ("aquifer_fluid_level_floodedness", -7, &[1.0]),
    ("aquifer_lava", -1, &[1.0]),
    ("aquifer_fluid_level_spread", -5, &[1.0]),
    ("pillar", -7, &[1.0, 1.0]),
    ("pillar_rareness", -8, &[1.0]),
    ("pillar_thickness", -8, &[1.0]),
    ("spaghetti_2d", -7, &[1.0]),
    ("spaghetti_2d_elevation", -8, &[1.0]),
    ("spaghetti_2d_modulator", -11, &[1.0]),
    ("spaghetti_2d_thickness", -11, &[1.0]),
    ("spaghetti_3d_1", -7, &[1.0]),
    ("spaghetti_3d_2", -7, &[1.0]),
    ("spaghetti_3d_rarity", -11, &[1.0]),
    ("spaghetti_3d_thickness", -8, &[1.0]),
    ("spaghetti_roughness", -5, &[1.0]),
    ("spaghetti_roughness_modulator", -8, &[1.0]),
    ("cave_entrance", -7, &[0.4, 0.5, 1.0]),
    ("cave_layer", -8, &[1.0]),
    ("cave_cheese", -8, &[0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 0.0, 2.0, 0.0]),
    ("ore_veininess", -8, &[1.0]),
    ("ore_vein_a", -7, &[1.0]),
    ("ore_vein_b", -7, &[1.0]),
    ("ore_gap", -5, &[1.0]),
    ("noodle", -8, &[1.0]),
    ("noodle_thickness", -8, &[1.0]),
    ("noodle_ridge_a", -7, &[1.0]),
    ("noodle_ridge_b", -7, &[1.0]),
    ("jagged", -16, &[1.0; 16]),
    ("surface", -6, &[1.0, 1.0, 1.0]),
    ("surface_secondary", -6, &[1.0, 1.0, 0.0, 1.0]),
    ("clay_bands_offset", -8, &[1.0]),
    ("badlands_pillar", -2, &[1.0, 1.0, 1.0, 1.0]),
    ("badlands_pillar_roof", -8, &[1.0]),
    ("badlands_surface", -6, &[1.0, 1.0, 1.0]),
    ("iceberg_pillar", -6, &[1.0, 1.0, 1.0, 1.0]),
    ("iceberg_pillar_roof", -3, &[1.0]),
    ("iceberg_surface", -6, &[1.0, 1.0, 1.0]),
    ("sulfur_cave_gradient", -5, &[1.0, 0.0, 1.0]),
    ("surface_swamp", -2, &[1.0]),
    ("calcite", -9, &[1.0, 1.0, 1.0, 1.0]),
    ("gravel", -8, &[1.0, 1.0, 1.0, 1.0]),
    ("powder_snow", -6, &[1.0, 1.0, 1.0, 1.0]),
    ("packed_ice", -7, &[1.0, 1.0, 1.0, 1.0]),
    ("ice", -4, &[1.0, 1.0, 1.0, 1.0]),
    ("soul_sand_layer", -8, &[1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.013333333333333334]),
    ("gravel_layer", -8, &[1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.013333333333333334]),
    ("patch", -5, &[1.0, 0.0, 0.0, 0.0, 0.0, 0.013333333333333334]),
    ("small_patch", -3, &[3.0]),
    ("netherrack", -3, &[1.0, 0.0, 0.0, 0.35]),
    ("nether_wart", -3, &[1.0, 0.0, 0.0, 0.9]),
    ("nether_state_selector", -4, &[1.0]),
];

pub fn noises() -> BTreeMap<ResourceLocation, NoiseParam> {
    NOISES
        .iter()
        .map(|(name, octave, amplitudes)| {
            let noise = NoiseParam::parity(*octave, amplitudes);
            (
                ResourceLocation::minecraft(name).expect("a hardcoded name"),
                noise,
            )
        })
        .collect()
}
