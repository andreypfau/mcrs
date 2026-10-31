use crate::density::{
    AMPLIFIED_TERRAIN, BASE_3D_NOISE_END, CELL_4, CELL_8, END_ISLANDS, LARGE_BIOMES_TERRAIN,
    OVERWORLD, OVERWORLD_TERRAIN, RIDGES, SLOPED_CHEESE_END, Terrain, full_noise,
    peaks_and_valleys, slide_end_like, slide_nether_like,
};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_resource::{ResourceLocationBuf, StaticResourceLocation, rl};
use mcrs_minecraft_worldgen_density::proto::ValueRange;
use mcrs_minecraft_worldgen_density::proto::build::Df;
use mcrs_minecraft_worldgen_density::router::{
    Aquifers, DebugFunction, NoiseGeneratorSettings, NoiseSettings, RouterFunctions,
    SpawnTargetPoint,
};
use std::collections::BTreeMap;
use std::num::NonZeroU32;

/// The density functions an overworld preset reads its climate from.
struct Climate {
    temperature: StaticResourceLocation,
    vegetation: StaticResourceLocation,
    continents: StaticResourceLocation,
    erosion: StaticResourceLocation,
}

const CLIMATE: Climate = Climate {
    temperature: rl!("minecraft:overworld/temperature"),
    vegetation: rl!("minecraft:overworld/vegetation"),
    continents: rl!("minecraft:overworld/continents"),
    erosion: rl!("minecraft:overworld/erosion"),
};

const LARGE_BIOMES_CLIMATE: Climate = Climate {
    temperature: rl!("minecraft:overworld_large_biomes/temperature"),
    vegetation: rl!("minecraft:overworld_large_biomes/vegetation"),
    continents: rl!("minecraft:overworld_large_biomes/continents"),
    erosion: rl!("minecraft:overworld_large_biomes/erosion"),
};

fn overworld(climate: &Climate, terrain: &Terrain) -> NoiseGeneratorSettings {
    let surface_level = Df::reference(terrain.preliminary_surface_level);
    let router = RouterFunctions {
        temperature: Df::reference(climate.temperature),
        vegetation: Df::reference(climate.vegetation),
        continents: Df::reference(climate.continents),
        erosion: Df::reference(climate.erosion),
        depth: Df::reference(terrain.depth),
        ridges: Df::reference(RIDGES),
        chunk_surface_level: Df::reference(terrain.chunk_surface_level),
        final_density: Df::reference(terrain.final_density),
    };
    let full_range = ValueRange::new(-1.0, 1.0);
    let spawn_target = [ValueRange::new(-1.0, -0.16), ValueRange::new(0.16, 1.0)]
        .map(|weirdness| -> SpawnTargetPoint {
            BTreeMap::from([
                (climate.temperature.into(), full_range.clone()),
                (climate.vegetation.into(), full_range.clone()),
                (climate.continents.into(), ValueRange::new(-0.11, 1.0)),
                (climate.erosion.into(), full_range.clone()),
                (RIDGES.into(), weirdness),
            ])
        })
        .into();
    let aquifers = Aquifers {
        barrier: Df::noise(rl!("minecraft:aquifer_barrier"), 1.0, 0.5),
        exclusion: (-0.225 - &router.erosion).min((&router.depth - 0.9).max(0.0)),
        fluid_level_floodedness: Df::noise(
            rl!("minecraft:aquifer_fluid_level_floodedness"),
            1.0,
            0.67,
        ),
        fluid_level_spread: Df::noise(
            rl!("minecraft:aquifer_fluid_level_spread"),
            1.0,
            0.7142857142857143,
        ),
        lava: Df::noise(rl!("minecraft:aquifer_lava"), 1.0, 1.0),
        surface_level: surface_level.clone(),
    };
    NoiseGeneratorSettings {
        spawn_target,
        aquifers: Some(aquifers),
        debug_functions: vec![
            DebugFunction::new("Final density", &router.final_density),
            DebugFunction::new("Temperature", &router.temperature),
            DebugFunction::new("Vegetation", &router.vegetation),
            DebugFunction::new("Continents", &router.continents),
            DebugFunction::new("Erosion", &router.erosion),
            DebugFunction::new("Depth", &router.depth),
            DebugFunction::new("Ridges", &router.ridges),
            DebugFunction::new("Peaks/Valleys", peaks_and_valleys(&router.ridges)),
            DebugFunction::new("Prelim Surface", surface_level),
        ],
        ..NoiseGeneratorSettings::new(
            OVERWORLD,
            BlockState::from(Block::Water),
            router,
            rl!("minecraft:overworld").into(),
            63,
        )
    }
}

fn nether() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(0, 128);
    let router = RouterFunctions {
        temperature: Df::noise(rl!("minecraft:nether/temperature"), 0.25, 0.0),
        vegetation: Df::noise(rl!("minecraft:nether/vegetation"), 0.25, 0.0),
        ..RouterFunctions::of_density(full_noise(slide_nether_like(bounds), CELL_4, CELL_8))
    };
    NoiseGeneratorSettings {
        legacy_random_source: true,
        debug_functions: vec![
            DebugFunction::new("N", &router.final_density),
            DebugFunction::new("T", &router.temperature),
            DebugFunction::new("V", &router.vegetation),
        ],
        ..NoiseGeneratorSettings::new(
            bounds,
            BlockState::from(Block::Lava),
            router,
            rl!("minecraft:nether").into(),
            32,
        )
    }
}

fn end() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(0, 128);
    let slide = slide_end_like(Df::reference(SLOPED_CHEESE_END), bounds);
    let router = RouterFunctions {
        erosion: Df::reference(END_ISLANDS),
        ..RouterFunctions::of_density(full_noise(slide, CELL_8, CELL_4))
    };
    NoiseGeneratorSettings {
        disable_mob_generation: true,
        legacy_random_source: true,
        debug_functions: vec![
            DebugFunction::new("N", &router.final_density),
            DebugFunction::new("IS", &router.erosion),
        ],
        ..NoiseGeneratorSettings::new(
            bounds,
            BlockState::from(Block::Air),
            router,
            rl!("minecraft:end").into(),
            0,
        )
    }
}

fn density_only(
    bounds: NoiseSettings,
    slide: Df,
    cell_size_xz: NonZeroU32,
    cell_size_y: NonZeroU32,
    material_rule: StaticResourceLocation,
    sea_level: i32,
) -> NoiseGeneratorSettings {
    let router = RouterFunctions::of_density(full_noise(slide, cell_size_xz, cell_size_y));
    NoiseGeneratorSettings {
        legacy_random_source: true,
        debug_functions: vec![DebugFunction::new("N", &router.final_density)],
        ..NoiseGeneratorSettings::new(
            bounds,
            BlockState::from(Block::Water),
            router,
            material_rule.into(),
            sea_level,
        )
    }
}

fn caves() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(-64, 192);
    density_only(
        bounds,
        slide_nether_like(bounds),
        CELL_4,
        CELL_8,
        rl!("minecraft:overworld_caves"),
        32,
    )
}

fn floating_islands() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(0, 256);
    let slide = slide_end_like(Df::reference(BASE_3D_NOISE_END), bounds);
    density_only(
        bounds,
        slide,
        CELL_8,
        CELL_4,
        rl!("minecraft:overworld_floating_islands"),
        -64,
    )
}

pub fn noise_settings() -> BTreeMap<ResourceLocationBuf, NoiseGeneratorSettings> {
    BTreeMap::from(
        [
            (
                rl!("minecraft:overworld"),
                overworld(&CLIMATE, &OVERWORLD_TERRAIN),
            ),
            (
                rl!("minecraft:large_biomes"),
                overworld(&LARGE_BIOMES_CLIMATE, &LARGE_BIOMES_TERRAIN),
            ),
            (
                rl!("minecraft:amplified"),
                overworld(&CLIMATE, &AMPLIFIED_TERRAIN),
            ),
            (rl!("minecraft:nether"), nether()),
            (rl!("minecraft:end"), end()),
            (rl!("minecraft:caves"), caves()),
            (rl!("minecraft:floating_islands"), floating_islands()),
        ]
        .map(|(name, settings)| (name.into(), settings)),
    )
}
