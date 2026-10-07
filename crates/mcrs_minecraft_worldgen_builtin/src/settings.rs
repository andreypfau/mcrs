use crate::density::{
    BASE_3D_NOISE_END, END_ISLANDS, OVERWORLD, RIDGES, SLOPED_CHEESE_END, full_noise,
    peaks_and_valleys, slide_end_like, slide_nether_like,
};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_density::proto::ValueRange;
use mcrs_minecraft_worldgen_density::proto::build::Df;
use mcrs_minecraft_worldgen_density::router::{
    Aquifers, DebugFunction, NoiseGeneratorSettings, NoiseSettings, RouterFunctions,
    SpawnTargetPoint,
};
use std::collections::BTreeMap;

fn id(name: &str) -> ResourceLocation {
    ResourceLocation::minecraft(name).expect("a hardcoded name")
}

fn overworld(climate: &str, terrain: &str) -> NoiseGeneratorSettings {
    let climate = |name: &str| format!("{climate}/{name}");
    let terrain = |name: &str| Df::reference(&format!("{terrain}/{name}"));
    let surface_level = terrain("preliminary_surface_level");
    let router = RouterFunctions {
        temperature: Df::reference(&climate("temperature")),
        vegetation: Df::reference(&climate("vegetation")),
        continents: Df::reference(&climate("continents")),
        erosion: Df::reference(&climate("erosion")),
        depth: terrain("depth"),
        ridges: Df::reference(RIDGES),
        chunk_surface_level: terrain("chunk_surface_level"),
        final_density: terrain("final_density"),
    };
    let full_range = ValueRange::new(-1.0, 1.0);
    let spawn_target = [ValueRange::new(-1.0, -0.16), ValueRange::new(0.16, 1.0)]
        .map(|weirdness| -> SpawnTargetPoint {
            BTreeMap::from([
                (id(&climate("temperature")), full_range.clone()),
                (id(&climate("vegetation")), full_range.clone()),
                (id(&climate("continents")), ValueRange::new(-0.11, 1.0)),
                (id(&climate("erosion")), full_range.clone()),
                (id(RIDGES), weirdness),
            ])
        })
        .into();
    let aquifers = Aquifers {
        barrier: Df::noise("aquifer_barrier", 1.0, 0.5),
        exclusion: (-0.225 - &router.erosion).min((&router.depth - 0.9).max(0.0)),
        fluid_level_floodedness: Df::noise("aquifer_fluid_level_floodedness", 1.0, 0.67),
        fluid_level_spread: Df::noise("aquifer_fluid_level_spread", 1.0, 0.7142857142857143),
        lava: Df::noise("aquifer_lava", 1.0, 1.0),
        surface_level: surface_level.clone(),
    };
    NoiseGeneratorSettings {
        spawn_target,
        aquifers: Some(aquifers),
        debug_functions: vec![
            DebugFunction::new("Final density", &router.final_density),
            DebugFunction::new("Temperature", &router.temperature),
            DebugFunction::new("Vegitation", &router.vegetation),
            DebugFunction::new("Contintents", &router.continents),
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
            id("overworld"),
            63,
        )
    }
}

fn nether() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(0, 128);
    let router = RouterFunctions {
        temperature: Df::noise("nether/temperature", 0.25, 0.0),
        vegetation: Df::noise("nether/vegetation", 0.25, 0.0),
        ..RouterFunctions::of_density(full_noise(slide_nether_like(bounds), 4, 8))
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
            id("nether"),
            32,
        )
    }
}

fn end() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(0, 128);
    let slide = slide_end_like(Df::reference(SLOPED_CHEESE_END), bounds);
    let router = RouterFunctions {
        erosion: Df::reference(END_ISLANDS),
        ..RouterFunctions::of_density(full_noise(slide, 8, 4))
    };
    NoiseGeneratorSettings {
        disable_mob_generation: true,
        legacy_random_source: true,
        debug_functions: vec![
            DebugFunction::new("N", &router.final_density),
            DebugFunction::new("IS", &router.erosion),
        ],
        ..NoiseGeneratorSettings::new(bounds, BlockState::from(Block::Air), router, id("end"), 0)
    }
}

fn density_only(
    bounds: NoiseSettings,
    slide: Df,
    cell_size_xz: u32,
    cell_size_y: u32,
    material_rule: &str,
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
            id(material_rule),
            sea_level,
        )
    }
}

fn caves() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(-64, 192);
    density_only(
        bounds,
        slide_nether_like(bounds),
        4,
        8,
        "overworld_caves",
        32,
    )
}

fn floating_islands() -> NoiseGeneratorSettings {
    let bounds = NoiseSettings::new(0, 256);
    let slide = slide_end_like(Df::reference(BASE_3D_NOISE_END), bounds);
    density_only(bounds, slide, 8, 4, "overworld_floating_islands", -64)
}

pub fn noise_settings() -> BTreeMap<ResourceLocation, NoiseGeneratorSettings> {
    BTreeMap::from([
        (id("overworld"), overworld("overworld", "overworld")),
        (
            id("large_biomes"),
            overworld("overworld_large_biomes", "overworld_large_biomes"),
        ),
        (
            id("amplified"),
            overworld("overworld", "overworld_amplified"),
        ),
        (id("nether"), nether()),
        (id("end"), end()),
        (id("caves"), caves()),
        (id("floating_islands"), floating_islands()),
    ])
}
