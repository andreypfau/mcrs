use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_density::proto::build::{Df, Functions};
use mcrs_minecraft_worldgen_density::router::{
    NoiseGeneratorSettings, NoiseSettings, RouterFunctions,
};
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;

const TEMPERATURE: &str = "beta/temperature";
const VEGETATION: &str = "beta/vegetation";
const CLIMATE_DETAIL: &str = "beta/climate_detail";
const CLIMATE_FACTOR: &str = "beta/climate_factor";
const SCALE: &str = "beta/scale";
const DEPTH_CURVE: &str = "beta/depth_curve";
const DEPTH: &str = "beta/depth";
const EFFECTIVE_SCALE: &str = "beta/effective_scale";
const FALLOFF: &str = "beta/falloff";
const TOP_SLIDE: &str = "beta/top_slide";

const CELL_WIDTH: f64 = 4.0;
const SIMPLEX_STRETCH: f64 = 1.5;

pub fn define(f: &mut Functions) {
    let detail = Df::reference(CLIMATE_DETAIL);
    let temperature = (Df::noise(
        "mcrs:beta/temperature",
        0.025f32 as f64 / SIMPLEX_STRETCH,
        0.0,
    ) * 0.15
        + 0.7)
        * 0.99
        + &detail * 0.01;
    let vegetation = (Df::noise(
        "mcrs:beta/vegetation",
        0.05f32 as f64 / SIMPLEX_STRETCH,
        0.0,
    ) * 0.15
        + 0.5)
        * 0.998
        + &detail * 0.002;
    let scale = (Df::noise("mcrs:beta/scale", 1.121 / CELL_WIDTH, 0.0) + 256.0)
        * (1.0 / 512.0)
        * Df::reference(CLIMATE_FACTOR);
    let raw_depth = Df::noise("mcrs:beta/depth", 200.0 / CELL_WIDTH, 0.0) * (1.0 / 8000.0);
    let curve = Df::reference(DEPTH_CURVE);
    let depth = Df::range_choice(
        &curve,
        ..0.0,
        (&curve * 0.5).clamp(-1.0, 0.0) * (1.0 / 1.4 / 2.0),
        curve.clone().clamp(0.0, 1.0) * (1.0 / 8.0),
    );
    let height_above_depth = Df::y_gradient(0, 128, 0.0, 16.0) - Df::reference(DEPTH);
    let falloff =
        (height_above_depth * 12.0 * Df::reference(EFFECTIVE_SCALE).reciprocal()).negate();

    f.define(
        CLIMATE_DETAIL,
        (Df::noise("mcrs:beta/climate_detail", 0.25 / SIMPLEX_STRETCH, 0.0) * 1.1 + 0.5).cache(),
    );
    f.define(
        TEMPERATURE,
        (1.0 - (1.0 - temperature).square()).clamp(0.0, 1.0).cache(),
    );
    f.define(VEGETATION, vegetation.clamp(0.0, 1.0).cache());
    f.define(
        CLIMATE_FACTOR,
        1.0 - (1.0 - Df::reference(TEMPERATURE) * Df::reference(VEGETATION))
            .square()
            .square(),
    );
    f.define(SCALE, (0.5 + scale.clamp(0.0, 1.0)).cache());
    f.define(
        DEPTH_CURVE,
        Df::range_choice(&raw_depth, ..0.0, &raw_depth * -0.3, &raw_depth) * 3.0 + -2.0,
    );
    f.define(DEPTH, (8.5 + depth * 4.25).cache());
    f.define(
        EFFECTIVE_SCALE,
        Df::range_choice(Df::reference(DEPTH), ..8.5, 0.5, Df::reference(SCALE)),
    );
    f.define(FALLOFF, falloff.quarter_negative() * 4.0);
    f.define(TOP_SLIDE, Df::y_gradient(104, 128, 0.0, 1.0));
}

/// The octaves of the beta noises. The sampler seeds them its own way from the
/// ids alone, so these entries are only what the loader finds under the ids.
pub fn noises() -> impl Iterator<Item = (ResourceLocation, NoiseParam)> {
    [
        ("climate_detail", -1, 2),
        ("depth", -15, 16),
        ("scale", -9, 10),
        ("temperature", -3, 4),
        ("vegetation", -3, 4),
    ]
    .into_iter()
    .map(|(name, base_octave, octave_count)| {
        let id = ResourceLocation::new("mcrs", &format!("beta/{name}")).expect("a hardcoded name");
        (id, NoiseParam::uniform(base_octave, octave_count))
    })
}

pub fn noise_settings() -> (ResourceLocation, NoiseGeneratorSettings) {
    let top_slide = Df::reference(TOP_SLIDE);
    let density =
        Df::old_blended_noise(0.25, 0.125, 80.0, 160.0, 8.0) * 128.0 + Df::reference(FALLOFF);
    let router = RouterFunctions {
        temperature: Df::reference(TEMPERATURE),
        vegetation: Df::reference(VEGETATION),
        ..RouterFunctions::of_density(
            (density * (1.0 - &top_slide) + &top_slide * -10.0).interpolated(4, 8),
        )
    };
    let settings = NoiseGeneratorSettings {
        default_block: Some(BlockState::bare(rl!("minecraft:stone").to_arc())),
        disable_mob_generation: true,
        legacy_random_source: true,
        ..NoiseGeneratorSettings::new(
            NoiseSettings::new(0, 128),
            BlockState::bare(rl!("minecraft:water").to_arc()).with("level", "0"),
            router,
            rl!("minecraft:overworld").to_arc(),
            64,
        )
    };
    (rl!("minecraft:beta").to_arc(), settings)
}
