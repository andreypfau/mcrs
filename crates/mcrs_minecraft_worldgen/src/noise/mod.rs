pub mod blended_noise;
pub mod improved_noise;
pub mod normal_noise;
pub mod octave_perlin_noise;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoiseParam {
    #[serde(rename = "firstOctave")]
    pub first_octave: i32,
    pub amplitudes: Vec<f64>,
}

impl Default for NoiseParam {
    fn default() -> Self {
        Self {
            first_octave: -1,
            amplitudes: vec![1.0],
        }
    }
}

impl NoiseParam {
    pub fn new(first_octave: i32, amplitudes: Vec<f64>) -> Self {
        Self {
            first_octave,
            amplitudes,
        }
    }
}

impl From<Noises> for NoiseParam {
    #[inline]
    fn from(noise: Noises) -> Self {
        noise.to_noise_param()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Copy, Serialize, Deserialize)]
pub enum Noises {
    #[serde(rename = "minecraft:temperature")]
    Temperature,
    #[serde(rename = "minecraft:vegetation")]
    Vegetation,
    #[serde(rename = "minecraft:continentalness")]
    Continentalness,
    #[serde(rename = "minecraft:erosion")]
    Erosion,
    #[serde(rename = "minecraft:temperature_large")]
    TemperatureLarge,
    #[serde(rename = "minecraft:vegetation_large")]
    VegetationLarge,
    #[serde(rename = "minecraft:continentalness_large")]
    ContinentalnessLarge,
    #[serde(rename = "minecraft:erosion_large")]
    ErosionLarge,
    #[serde(rename = "minecraft:ridge")]
    Ridge,
    #[serde(rename = "minecraft:offset")]
    Offset,
    #[serde(rename = "minecraft:aquifer_barrier")]
    AquiferBarrier,
    #[serde(rename = "minecraft:aquifer_fluid_level_floodedness")]
    AquiferFluidLevelFloodedness,
    #[serde(rename = "minecraft:aquifer_lava")]
    AquiferLava,
    #[serde(rename = "minecraft:aquifer_fluid_level_spread")]
    AquiferFluidLevelSpread,
    #[serde(rename = "minecraft:pillar")]
    Pillar,
    #[serde(rename = "minecraft:pillar_rareness")]
    PillarRareness,
    #[serde(rename = "minecraft:pillar_thickness")]
    PillarThickness,
    #[serde(rename = "minecraft:spaghetti_2d")]
    Spaghetti2D,
    #[serde(rename = "minecraft:spaghetti_2d_elevation")]
    Spaghetti2DElevation,
    #[serde(rename = "minecraft:spaghetti_2d_modulator")]
    Spaghetti2DModulator,
    #[serde(rename = "minecraft:spaghetti_2d_thickness")]
    Spaghetti2DThickness,
    #[serde(rename = "minecraft:spaghetti_3d_1")]
    Spaghetti3D1,
    #[serde(rename = "minecraft:spaghetti_3d_2")]
    Spaghetti3D2,
    #[serde(rename = "minecraft:spaghetti_3d_rarity")]
    Spaghetti3DRarity,
    #[serde(rename = "minecraft:spaghetti_3d_thickness")]
    Spaghetti3DThickness,
    #[serde(rename = "minecraft:spaghetti_roughness")]
    SpaghettiRoughness,
    #[serde(rename = "minecraft:spaghetti_roughness_modulator")]
    SpaghettiRoughnessModulator,
    #[serde(rename = "minecraft:cave_entrance")]
    CaveEntrance,
    #[serde(rename = "minecraft:cave_layer")]
    CaveLayer,
    #[serde(rename = "minecraft:cave_cheese")]
    CaveCheese,
    #[serde(rename = "minecraft:ore_veininess")]
    OreVeininess,
    #[serde(rename = "minecraft:ore_vein_a")]
    OreVeinA,
    #[serde(rename = "minecraft:ore_vein_b")]
    OreVeinB,
    #[serde(rename = "minecraft:ore_gap")]
    OreGap,
    #[serde(rename = "minecraft:noodle")]
    Noodle,
    #[serde(rename = "minecraft:noodle_thickness")]
    NoodleThickness,
    #[serde(rename = "minecraft:noodle_ridge_a")]
    NoodleRidgeA,
    #[serde(rename = "minecraft:noodle_ridge_b")]
    NoodleRidgeB,
    #[serde(rename = "minecraft:jagged")]
    Jagged,
    #[serde(rename = "minecraft:surface")]
    Surface,
    #[serde(rename = "minecraft:surface_secondary")]
    SurfaceSecondary,
    #[serde(rename = "minecraft:clay_bands_offset")]
    ClayBandsOffset,
    #[serde(rename = "minecraft:badlands_pillar")]
    BadlandsPillar,
    #[serde(rename = "minecraft:badlands_pillar_roof")]
    BadlandsPillarRoof,
    #[serde(rename = "minecraft:badlands_surface")]
    BadlandsSurface,
    #[serde(rename = "minecraft:iceberg_pillar")]
    IcebergPillar,
    #[serde(rename = "minecraft:iceberg_pillar_roof")]
    IcebergPillarRoof,
    #[serde(rename = "minecraft:iceberg_surface")]
    IcebergSurface,
    #[serde(rename = "minecraft:surface_swamp")]
    SurfaceSwamp,
    #[serde(rename = "minecraft:calcite")]
    Calcite,
    #[serde(rename = "minecraft:gravel")]
    Gravel,
    #[serde(rename = "minecraft:powder_snow")]
    PowderSnow,
    #[serde(rename = "minecraft:packed_ice")]
    PackedIce,
    #[serde(rename = "minecraft:ice")]
    Ice,
    #[serde(rename = "minecraft:soul_sand_layer")]
    SoulSandLayer,
    #[serde(rename = "minecraft:gravel_layer")]
    GravelLayer,
    #[serde(rename = "minecraft:patch")]
    Patch,
    #[serde(rename = "minecraft:netherrack")]
    Netherrack,
    #[serde(rename = "minecraft:nether_wart")]
    NetherWart,
    #[serde(rename = "minecraft:nether_state_selector")]
    NetherStateSelector,
}

impl Noises {
    pub fn to_noise_param(self) -> NoiseParam {
        match self {
            Noises::Temperature => NoiseParam::new(-10, vec![1.5, 0.0, 1.0, 0.0, 0.0, 0.0]),
            Noises::Vegetation => NoiseParam::new(-8, vec![1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
            Noises::Continentalness => {
                NoiseParam::new(-9, vec![1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0])
            }
            Noises::Erosion => NoiseParam::new(-9, vec![1.0, 1.0, 0.0, 1.0, 1.0]),
            Noises::TemperatureLarge => NoiseParam::new(-12, vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            Noises::VegetationLarge => NoiseParam::new(-10, vec![1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
            Noises::ContinentalnessLarge => {
                NoiseParam::new(-11, vec![1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0])
            }
            Noises::ErosionLarge => NoiseParam::new(-11, vec![1.0, 1.0, 0.0, 1.0, 1.0]),
            Noises::Ridge => NoiseParam::new(-7, vec![1.0, 2.0, 1.0, 0.0, 0.0, 0.0]),
            Noises::Offset => NoiseParam::new(-3, vec![1.0, 1.0, 1.0, 0.0]),
            Noises::AquiferBarrier => NoiseParam::new(-3, vec![1.0]),
            Noises::AquiferFluidLevelFloodedness => NoiseParam::new(-7, vec![1.0]),
            Noises::AquiferLava => NoiseParam::new(-1, vec![1.0]),
            Noises::AquiferFluidLevelSpread => NoiseParam::new(-5, vec![1.0]),
            Noises::Pillar => NoiseParam::new(-7, vec![1.0, 1.0]),
            Noises::PillarRareness => NoiseParam::new(-8, vec![1.0]),
            Noises::PillarThickness => NoiseParam::new(-8, vec![1.0]),
            Noises::Spaghetti2D => NoiseParam::new(-7, vec![1.0]),
            Noises::Spaghetti2DElevation => NoiseParam::new(-8, vec![1.0]),
            Noises::Spaghetti2DModulator => NoiseParam::new(-11, vec![1.0]),
            Noises::Spaghetti2DThickness => NoiseParam::new(-11, vec![1.0]),
            Noises::Spaghetti3D1 => NoiseParam::new(-7, vec![1.0]),
            Noises::Spaghetti3D2 => NoiseParam::new(-7, vec![1.0]),
            Noises::Spaghetti3DRarity => NoiseParam::new(-11, vec![1.0]),
            Noises::Spaghetti3DThickness => NoiseParam::new(-8, vec![1.0]),
            Noises::SpaghettiRoughness => NoiseParam::new(-5, vec![1.0]),
            Noises::SpaghettiRoughnessModulator => NoiseParam::new(-8, vec![1.0]),
            Noises::CaveEntrance => NoiseParam::new(-7, vec![0.4, 0.5, 1.0]),
            Noises::CaveLayer => NoiseParam::new(-8, vec![1.0]),
            Noises::CaveCheese => {
                NoiseParam::new(-8, vec![0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 0.0, 2.0, 0.0])
            }
            Noises::OreVeininess => NoiseParam::new(-8, vec![1.0]),
            Noises::OreVeinA => NoiseParam::new(-7, vec![1.0]),
            Noises::OreVeinB => NoiseParam::new(-7, vec![1.0]),
            Noises::OreGap => NoiseParam::new(-5, vec![1.0]),
            Noises::Noodle => NoiseParam::new(-8, vec![1.0]),
            Noises::NoodleThickness => NoiseParam::new(-8, vec![1.0]),
            Noises::NoodleRidgeA => NoiseParam::new(-7, vec![1.0]),
            Noises::NoodleRidgeB => NoiseParam::new(-7, vec![1.0]),
            Noises::Jagged => NoiseParam::new(
                -16,
                vec![
                    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                ],
            ),
            Noises::Surface => NoiseParam::new(-6, vec![1.0, 1.0, 1.0]),
            Noises::SurfaceSecondary => NoiseParam::new(-6, vec![1.0, 1.0, 0.0, 1.0]),
            Noises::ClayBandsOffset => NoiseParam::new(-8, vec![1.0]),
            Noises::BadlandsPillar => NoiseParam::new(-2, vec![1.0, 1.0, 1.0, 1.0]),
            Noises::BadlandsPillarRoof => NoiseParam::new(-8, vec![1.0]),
            Noises::BadlandsSurface => NoiseParam::new(-6, vec![1.0, 1.0, 1.0]),
            Noises::IcebergPillar => NoiseParam::new(-6, vec![1.0, 1.0, 1.0, 1.0]),
            Noises::IcebergPillarRoof => NoiseParam::new(-3, vec![1.0]),
            Noises::IcebergSurface => NoiseParam::new(-6, vec![1.0, 1.0, 1.0]),
            Noises::SurfaceSwamp => NoiseParam::new(-2, vec![1.0]),
            Noises::Calcite => NoiseParam::new(-9, vec![1.0, 1.0, 1.0, 1.0]),
            Noises::Gravel => NoiseParam::new(-8, vec![1.0, 1.0, 1.0, 1.0]),
            Noises::PowderSnow => NoiseParam::new(-6, vec![1.0, 1.0, 1.0, 1.0]),
            Noises::PackedIce => NoiseParam::new(-7, vec![1.0, 1.0, 1.0, 1.0]),
            Noises::Ice => NoiseParam::new(-4, vec![1.0, 1.0, 1.0, 1.0]),
            Noises::SoulSandLayer => {
                NoiseParam::new(-8, vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0 / 75.0])
            }
            Noises::GravelLayer => {
                NoiseParam::new(-8, vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0 / 75.0])
            }
            Noises::Patch => NoiseParam::new(-5, vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0 / 75.0]),
            Noises::Netherrack => NoiseParam::new(-3, vec![1.0, 0.0, 0.0, 0.35]),
            Noises::NetherWart => NoiseParam::new(-3, vec![1.0, 0.0, 0.0, 0.9]),
            Noises::NetherStateSelector => NoiseParam::new(-4, vec![1.0]),
        }
    }
}
