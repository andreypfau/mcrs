use crate::terrain::{self, Coordinates};
use mcrs_minecraft_worldgen_density::node::distance::DistanceMetric;
use mcrs_minecraft_worldgen_density::proto::MAX_REASONABLE_NOISE_VALUE;
use mcrs_minecraft_worldgen_density::proto::build::{Df, Functions};
use mcrs_minecraft_worldgen_density::router::NoiseSettings;
use std::ops::RangeInclusive;

pub(crate) const OVERWORLD: NoiseSettings = NoiseSettings::new(-64, 384);
const OVERWORLD_TOP: i32 = OVERWORLD.min_y + OVERWORLD.height as i32;

const Y: &str = "y";
pub(crate) const RIDGES: &str = "overworld/ridges";
const RIDGES_FOLDED: &str = "overworld/ridges_folded";
const BASE_3D_NOISE_OVERWORLD: &str = "overworld/base_3d_noise";
const BASE_3D_NOISE_NETHER: &str = "nether/base_3d_noise";
pub(crate) const BASE_3D_NOISE_END: &str = "end/base_3d_noise";
pub(crate) const END_ISLANDS: &str = "end/islands";
pub(crate) const SLOPED_CHEESE_END: &str = "end/sloped_cheese";
const SPAGHETTI_ROUGHNESS_FUNCTION: &str = "overworld/caves/spaghetti_roughness_function";
const ENTRANCES: &str = "overworld/caves/entrances";
const NOODLE: &str = "overworld/caves/noodle";
const PILLARS: &str = "overworld/caves/pillars";
const SPAGHETTI_2D_THICKNESS_MODULATOR: &str = "overworld/caves/spaghetti_2d_thickness_modulator";
const SPAGHETTI_2D: &str = "overworld/caves/spaghetti_2d";

const COPPER_VEIN: RangeInclusive<i32> = 0..=50;
const IRON_VEIN: RangeInclusive<i32> = -60..=-8;

pub fn define(f: &mut Functions) {
    f.define("zero", Df::ZERO);
    f.define(Y, Df::y());
    ore_veins(f);
    let shift_x = f.define("shift_x", Df::shift_a("offset").cache());
    let shift_z = f.define("shift_z", Df::shift_b("offset").cache());
    f.define(
        BASE_3D_NOISE_OVERWORLD,
        Df::old_blended_noise(0.25, 0.125, 80.0, 160.0, 8.0),
    );
    f.define(
        BASE_3D_NOISE_NETHER,
        Df::old_blended_noise(0.25, 0.375, 80.0, 60.0, 8.0),
    );
    f.define(
        BASE_3D_NOISE_END,
        Df::old_blended_noise(0.25, 0.25, 80.0, 160.0, 4.0),
    );

    let climate = |noise: &str| Df::shifted_noise_2d(&shift_x, &shift_z, 0.25, noise);
    f.define("overworld/temperature", climate("temperature"));
    f.define(
        "overworld_large_biomes/temperature",
        climate("temperature_large"),
    );
    f.define("overworld/vegetation", climate("vegetation"));
    f.define(
        "overworld_large_biomes/vegetation",
        climate("vegetation_large"),
    );
    let continents = f.define("overworld/continents", climate("continentalness").cache());
    let erosion = f.define("overworld/erosion", climate("erosion").cache());
    let ridge = f.define(RIDGES, climate("ridge").cache());
    f.define(RIDGES_FOLDED, peaks_and_valleys(&ridge));
    let jagged = Df::noise("jagged", 1500.0, 0.0);
    terrain_noises(
        f,
        &jagged,
        continents.clone(),
        erosion.clone(),
        "overworld",
        false,
    );
    let continents_large = f.define(
        "overworld_large_biomes/continents",
        climate("continentalness_large").cache(),
    );
    let erosion_large = f.define(
        "overworld_large_biomes/erosion",
        climate("erosion_large").cache(),
    );
    terrain_noises(
        f,
        &jagged,
        continents_large,
        erosion_large,
        "overworld_large_biomes",
        false,
    );
    terrain_noises(f, &jagged, continents, erosion, "overworld_amplified", true);

    let end_islands = f.define(END_ISLANDS, end_islands());
    f.define(
        SLOPED_CHEESE_END,
        end_islands + Df::reference(BASE_3D_NOISE_END),
    );
    f.define(SPAGHETTI_ROUGHNESS_FUNCTION, spaghetti_roughness());
    f.define(
        SPAGHETTI_2D_THICKNESS_MODULATOR,
        Df::mapped_noise("spaghetti_2d_thickness", 2.0, 1.0, -0.6, -1.3).cache(),
    );
    f.define(SPAGHETTI_2D, spaghetti_2d());
    f.define(ENTRANCES, entrances());
    f.define(NOODLE, noodle());
    f.define(PILLARS, pillars());
}

fn end_islands() -> Df {
    let distance = Df::distance_to_point([0, 0, 0], DistanceMetric::Euclidean);
    let main_island = ((100.0 - distance).clamp(-100.0, 80.0) - 8.0) * 0.0078125;
    main_island.slice_y(0).max(Df::end_outer_islands()).cache()
}

fn terrain_noises(
    f: &mut Functions,
    jagged: &Df,
    continents: Df,
    erosion: Df,
    prefix: &str,
    amplified: bool,
) {
    let name = |name: &str| format!("{prefix}/{name}");
    let coordinates = Coordinates {
        continents,
        erosion,
        weirdness: Df::reference(RIDGES),
        ridges: Df::reference(RIDGES_FOLDED),
    };
    let offset = f.define(
        &name("offset"),
        (-0.50375 + Df::from(terrain::offset(&coordinates, amplified))).blended(Df::blend_offset()),
    );
    let factor = f.define(
        &name("factor"),
        Df::from(terrain::factor(&coordinates, amplified)).blended(10.0),
    );
    let depth = f.define(&name("depth"), offset_to_depth(&offset));
    let unscaled_jaggedness = f.define(
        &name("jaggedness"),
        Df::from(terrain::jaggedness(&coordinates, amplified)).blended(0.0),
    );
    let jaggedness = (unscaled_jaggedness * jagged.clone().half_negative()).cache();
    let initial_density = noise_gradient_density(&factor, depth + jaggedness);
    let sloped_cheese = f.define(
        &name("sloped_cheese"),
        (initial_density + Df::reference(BASE_3D_NOISE_OVERWORLD)).cache(),
    );
    let surface_level = f.define(
        &name("preliminary_surface_level"),
        preliminary_surface_level(&offset, &factor, amplified),
    );
    f.define(
        &name("chunk_surface_level"),
        surface_level.interpolated(16, 1),
    );
    let surface_with_entrances = sloped_cheese.clone().min(Df::reference(ENTRANCES) * 5.0);
    let caves = Df::range_choice(
        &sloped_cheese,
        ..1.5625,
        surface_with_entrances,
        underground(&sloped_cheese),
    );
    f.define(
        &name("final_density"),
        post_process(slide_overworld(amplified, caves), 4, 8).min(Df::reference(NOODLE))
            + Df::beardifier(),
    );
}

fn offset_to_depth(offset: &Df) -> Df {
    Df::y_gradient(OVERWORLD.min_y, OVERWORLD_TOP, 1.5, -1.5) + offset
}

pub fn peaks_and_valleys(weirdness: &Df) -> Df {
    ((weirdness.clone().abs() + -0.6666667).abs() + -0.33333334) * -3.0
}

fn spaghetti_roughness() -> Df {
    let roughness = Df::noise("spaghetti_roughness", 1.0, 1.0);
    let modulator = Df::mapped_noise("spaghetti_roughness_modulator", 1.0, 1.0, 0.0, -0.1);
    (modulator * (roughness.abs() + -0.4)).cache()
}

fn entrances() -> Df {
    let rarity = Df::noise("spaghetti_3d_rarity", 2.0, 1.0).cache();
    let thickness = Df::mapped_noise("spaghetti_3d_thickness", 1.0, 1.0, -0.065, -0.088);
    let cave =
        |noise: &str| Df::rarity_select(&rarity, noise, &[-0.5, 0.0, 0.5], &[0.75, 1.0, 1.5, 2.0]);
    let spaghetti_3d =
        (cave("spaghetti_3d_1").max(cave("spaghetti_3d_2")) + thickness).clamp(-1.0, 1.0);
    let big_entrances =
        Df::noise("cave_entrance", 0.75, 0.5) + 0.37 + Df::y_gradient(-10, 30, 0.3, 0.0);
    big_entrances
        .min(Df::reference(SPAGHETTI_ROUGHNESS_FUNCTION) + spaghetti_3d)
        .cache()
}

fn noodle() -> Df {
    let y = Df::reference(Y);
    let limited = |input: Df, out_of_range: f32| {
        input
            .y_limited(&y, -60..=OVERWORLD_TOP, out_of_range)
            .interpolated(4, 8)
    };
    let toggle = limited(Df::noise("noodle", 1.0, 1.0), -1.0);
    let thickness = limited(
        Df::mapped_noise("noodle_thickness", 1.0, 1.0, -0.05, -0.1),
        0.0,
    );
    let ridge_frequency = 2.6666666666666665;
    let ridge =
        |noise: &str| limited(Df::noise(noise, ridge_frequency, ridge_frequency), 0.0).abs();
    let ridged = ridge("noodle_ridge_a").max(ridge("noodle_ridge_b")) * 1.5;
    Df::range_choice(toggle, ..0.0, 64.0, thickness + ridged)
}

fn pillars() -> Df {
    let rareness = Df::mapped_noise("pillar_rareness", 1.0, 1.0, 0.0, -2.0);
    let thickness = Df::mapped_noise("pillar_thickness", 1.0, 1.0, 0.0, 1.1);
    ((Df::noise("pillar", 25.0, 0.3) * 2.0 + rareness) * thickness.cube()).cache()
}

fn spaghetti_2d() -> Df {
    let cave = Df::rarity_select(
        Df::noise("spaghetti_2d_modulator", 2.0, 1.0),
        "spaghetti_2d",
        &[-0.75, -0.5, 0.5, 0.75],
        &[0.5, 0.75, 1.0, 2.0, 3.0],
    );
    let lowest_cell = OVERWORLD.min_y.div_euclid(8) as f32;
    let elevation = Df::mapped_noise("spaghetti_2d_elevation", 1.0, 0.0, lowest_cell, 8.0);
    let thickness = Df::reference(SPAGHETTI_2D_THICKNESS_MODULATOR);
    let sloped =
        (elevation.cache() + Df::y_gradient(OVERWORLD.min_y, OVERWORLD_TOP, 8.0, -40.0)).abs();
    let layer_ridged = (sloped + &thickness).cube();
    (cave + thickness * 0.083)
        .max(layer_ridged)
        .clamp(-1.0, 1.0)
}

fn underground(sloped_cheese: &Df) -> Df {
    let layerized_caverns = Df::noise("cave_layer", 1.0, 8.0).square() * 4.0;
    let cheese = Df::noise("cave_cheese", 1.0, 0.6666666666666666);
    let solidified_cheese_with_top_slide =
        (cheese + 0.27).clamp(-1.0, 1.0) + (sloped_cheese * -0.64 + 1.5).clamp(0.0, 0.5);
    let subtractions = (layerized_caverns + solidified_cheese_with_top_slide)
        .min(Df::reference(ENTRANCES))
        .min(Df::reference(SPAGHETTI_2D) + Df::reference(SPAGHETTI_ROUGHNESS_FUNCTION));
    let pillars = Df::reference(PILLARS);
    subtractions.max(Df::range_choice(
        &pillars,
        ..0.03,
        -MAX_REASONABLE_NOISE_VALUE,
        &pillars,
    ))
}

fn post_process(slide: Df, cell_size_xz: u32, cell_size_y: u32) -> Df {
    (slide.blend_density() * 0.64)
        .interpolated(cell_size_xz, cell_size_y)
        .squeeze()
}

pub fn full_noise(slide: Df, cell_size_xz: u32, cell_size_y: u32) -> Df {
    post_process(slide, cell_size_xz, cell_size_y) + Df::beardifier()
}

fn ore_veins(f: &mut Functions) {
    let y = Df::reference(Y);
    let lowest = *COPPER_VEIN.start().min(IRON_VEIN.start());
    let highest = *COPPER_VEIN.end().max(IRON_VEIN.end());
    let cells = lowest.div_euclid(8) * 8..=(highest.div_euclid(8) + 1) * 8;
    let limited = |input: Df, out_of_range: f32| {
        input
            .y_limited(&y, cells.clone(), out_of_range)
            .interpolated(4, 8)
    };
    let toggle = f.define(
        "overworld/ore_vein/toggle",
        limited(Df::noise("ore_veininess", 1.5, 1.5), 0.0).cache(),
    );
    f.define(
        "overworld/ore_vein/richness",
        toggle.clone().abs().clamped_map(0.4, 0.6, 0.1, 0.3),
    );
    let vein = |noise: &str| limited(Df::noise(noise, 4.0, 4.0), 1.0).abs();
    let no_vein_near_toggle_zero = Df::range_choice(
        &toggle,
        -0.4..0.4,
        -1.0,
        0.08 - vein("ore_vein_a").max(vein("ore_vein_b")),
    );
    let mask = f.define("overworld/ore_vein/mask", no_vein_near_toggle_zero.cache());
    f.define(
        "overworld/ore_vein/copper_density",
        ore_vein_density(COPPER_VEIN, &y, toggle.clone(), &mask),
    );
    f.define(
        "overworld/ore_vein/iron_density",
        ore_vein_density(IRON_VEIN, &y, toggle.negate(), &mask),
    );
    f.define(
        "overworld/ore_vein/gap",
        -0.3 - Df::noise("ore_gap", 1.0, 1.0),
    );
}

fn ore_vein_density(vein: RangeInclusive<i32>, y: &Df, veininess: Df, mask: &Df) -> Df {
    let no_vein = Df::from(-1.0);
    let (min_y, max_y) = (*vein.start() as f32, *vein.end() as f32);
    let distance_from_edge = (Df::from(max_y) - y).min(y - min_y);
    let edge_roundoff = distance_from_edge.clamped_map(0.0, 20.0, -0.2, 0.0);
    let rich_enough = Df::range_choice(veininess - 0.4 + edge_roundoff, 0.0.., 0.7, &no_vein);
    let inside_mask = Df::range_choice(mask, 0.0.., rich_enough, &no_vein);
    Df::range_choice(y, min_y..max_y, inside_mask, &no_vein)
}

fn slide_overworld(amplified: bool, caves: Df) -> Df {
    if amplified {
        caves.slide(OVERWORLD, (16, 0, -0.078125), (0, 24, 0.4))
    } else {
        caves.slide(OVERWORLD, (80, 64, -0.078125), (0, 24, 0.1171875))
    }
}

pub fn slide_nether_like(bounds: NoiseSettings) -> Df {
    Df::reference(BASE_3D_NOISE_NETHER).slide(bounds, (24, 0, 0.9375), (-8, 24, 2.5))
}

pub fn slide_end_like(caves: Df, bounds: NoiseSettings) -> Df {
    caves.slide(bounds, (72, -184, -23.4375), (4, 32, -0.234375))
}

fn noise_gradient_density(factor: &Df, depth_with_jaggedness: Df) -> Df {
    (depth_with_jaggedness * factor).quarter_negative() * 4.0
}

fn preliminary_surface_level(offset: &Df, factor: &Df, amplified: bool) -> Df {
    let upper_bound = (0.2734375 / factor - offset)
        .remap(1.5, -1.5, -64.0, 320.0)
        .clamp(-40.0, 320.0);
    let gradient = noise_gradient_density(factor, offset_to_depth(offset));
    let density = slide_overworld(amplified, (gradient + -0.703125).clamp(-64.0, 64.0)) + -0.390625;
    Df::find_top_surface(density, upper_bound, OVERWORLD.min_y, 8)
}
