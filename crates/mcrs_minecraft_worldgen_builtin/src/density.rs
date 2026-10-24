use crate::terrain::{self, Coordinates};
use mcrs_minecraft_core::{StaticResourceLocation, rl};
use mcrs_minecraft_worldgen_density::node::distance::DistanceMetric;
use mcrs_minecraft_worldgen_density::proto::MAX_REASONABLE_NOISE_VALUE;
use mcrs_minecraft_worldgen_density::proto::build::{Df, Functions};
use mcrs_minecraft_worldgen_density::router::NoiseSettings;
use std::num::NonZeroU32;
use std::ops::RangeInclusive;

pub(crate) const OVERWORLD: NoiseSettings = NoiseSettings::new(-64, 384);
const OVERWORLD_TOP: i32 = OVERWORLD.min_y + OVERWORLD.height as i32;

const Y: StaticResourceLocation = rl!("minecraft:y");
pub(crate) const RIDGES: StaticResourceLocation = rl!("minecraft:overworld/ridges");
const RIDGES_FOLDED: StaticResourceLocation = rl!("minecraft:overworld/ridges_folded");
const BASE_3D_NOISE_OVERWORLD: StaticResourceLocation = rl!("minecraft:overworld/base_3d_noise");
const BASE_3D_NOISE_NETHER: StaticResourceLocation = rl!("minecraft:nether/base_3d_noise");
pub(crate) const BASE_3D_NOISE_END: StaticResourceLocation = rl!("minecraft:end/base_3d_noise");
pub(crate) const END_ISLANDS: StaticResourceLocation = rl!("minecraft:end/islands");
pub(crate) const SLOPED_CHEESE_END: StaticResourceLocation = rl!("minecraft:end/sloped_cheese");
const SPAGHETTI_ROUGHNESS_FUNCTION: StaticResourceLocation =
    rl!("minecraft:overworld/caves/spaghetti_roughness_function");
const ENTRANCES: StaticResourceLocation = rl!("minecraft:overworld/caves/entrances");
const NOODLE: StaticResourceLocation = rl!("minecraft:overworld/caves/noodle");
const PILLARS: StaticResourceLocation = rl!("minecraft:overworld/caves/pillars");
const SPAGHETTI_2D_THICKNESS_MODULATOR: StaticResourceLocation =
    rl!("minecraft:overworld/caves/spaghetti_2d_thickness_modulator");
const SPAGHETTI_2D: StaticResourceLocation = rl!("minecraft:overworld/caves/spaghetti_2d");

/// The density functions one overworld preset defines for its terrain.
pub(crate) struct Terrain {
    pub(crate) offset: StaticResourceLocation,
    pub(crate) factor: StaticResourceLocation,
    pub(crate) depth: StaticResourceLocation,
    pub(crate) jaggedness: StaticResourceLocation,
    pub(crate) sloped_cheese: StaticResourceLocation,
    pub(crate) preliminary_surface_level: StaticResourceLocation,
    pub(crate) chunk_surface_level: StaticResourceLocation,
    pub(crate) final_density: StaticResourceLocation,
}

pub(crate) const OVERWORLD_TERRAIN: Terrain = Terrain {
    offset: rl!("minecraft:overworld/offset"),
    factor: rl!("minecraft:overworld/factor"),
    depth: rl!("minecraft:overworld/depth"),
    jaggedness: rl!("minecraft:overworld/jaggedness"),
    sloped_cheese: rl!("minecraft:overworld/sloped_cheese"),
    preliminary_surface_level: rl!("minecraft:overworld/preliminary_surface_level"),
    chunk_surface_level: rl!("minecraft:overworld/chunk_surface_level"),
    final_density: rl!("minecraft:overworld/final_density"),
};

pub(crate) const LARGE_BIOMES_TERRAIN: Terrain = Terrain {
    offset: rl!("minecraft:overworld_large_biomes/offset"),
    factor: rl!("minecraft:overworld_large_biomes/factor"),
    depth: rl!("minecraft:overworld_large_biomes/depth"),
    jaggedness: rl!("minecraft:overworld_large_biomes/jaggedness"),
    sloped_cheese: rl!("minecraft:overworld_large_biomes/sloped_cheese"),
    preliminary_surface_level: rl!("minecraft:overworld_large_biomes/preliminary_surface_level"),
    chunk_surface_level: rl!("minecraft:overworld_large_biomes/chunk_surface_level"),
    final_density: rl!("minecraft:overworld_large_biomes/final_density"),
};

pub(crate) const AMPLIFIED_TERRAIN: Terrain = Terrain {
    offset: rl!("minecraft:overworld_amplified/offset"),
    factor: rl!("minecraft:overworld_amplified/factor"),
    depth: rl!("minecraft:overworld_amplified/depth"),
    jaggedness: rl!("minecraft:overworld_amplified/jaggedness"),
    sloped_cheese: rl!("minecraft:overworld_amplified/sloped_cheese"),
    preliminary_surface_level: rl!("minecraft:overworld_amplified/preliminary_surface_level"),
    chunk_surface_level: rl!("minecraft:overworld_amplified/chunk_surface_level"),
    final_density: rl!("minecraft:overworld_amplified/final_density"),
};

pub(crate) const CELL_1: NonZeroU32 = NonZeroU32::MIN;
pub(crate) const CELL_4: NonZeroU32 = NonZeroU32::new(4).unwrap();
pub(crate) const CELL_8: NonZeroU32 = NonZeroU32::new(8).unwrap();
const CELL_16: NonZeroU32 = NonZeroU32::new(16).unwrap();

const COPPER_VEIN: RangeInclusive<i32> = 0..=50;
const IRON_VEIN: RangeInclusive<i32> = -60..=-8;

pub fn define(f: &mut Functions) {
    f.define(rl!("minecraft:zero"), Df::ZERO);
    f.define(Y, Df::y());
    ore_veins(f);
    let shift_x = f.define(
        rl!("minecraft:shift_x"),
        Df::shift_a(rl!("minecraft:offset")).cache(),
    );
    let shift_z = f.define(
        rl!("minecraft:shift_z"),
        Df::shift_b(rl!("minecraft:offset")).cache(),
    );
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

    let climate =
        |noise: StaticResourceLocation| Df::shifted_noise_2d(&shift_x, &shift_z, 0.25, noise);
    f.define(
        rl!("minecraft:overworld/temperature"),
        climate(rl!("minecraft:temperature")),
    );
    f.define(
        rl!("minecraft:overworld_large_biomes/temperature"),
        climate(rl!("minecraft:temperature_large")),
    );
    f.define(
        rl!("minecraft:overworld/vegetation"),
        climate(rl!("minecraft:vegetation")),
    );
    f.define(
        rl!("minecraft:overworld_large_biomes/vegetation"),
        climate(rl!("minecraft:vegetation_large")),
    );
    let continents = f.define(
        rl!("minecraft:overworld/continents"),
        climate(rl!("minecraft:continentalness")).cache(),
    );
    let erosion = f.define(
        rl!("minecraft:overworld/erosion"),
        climate(rl!("minecraft:erosion")).cache(),
    );
    let ridge = f.define(RIDGES, climate(rl!("minecraft:ridge")).cache());
    f.define(RIDGES_FOLDED, peaks_and_valleys(&ridge));
    let jagged = Df::noise(rl!("minecraft:jagged"), 1500.0, 0.0);
    terrain_noises(
        f,
        &jagged,
        continents.clone(),
        erosion.clone(),
        &OVERWORLD_TERRAIN,
        false,
    );
    let continents_large = f.define(
        rl!("minecraft:overworld_large_biomes/continents"),
        climate(rl!("minecraft:continentalness_large")).cache(),
    );
    let erosion_large = f.define(
        rl!("minecraft:overworld_large_biomes/erosion"),
        climate(rl!("minecraft:erosion_large")).cache(),
    );
    terrain_noises(
        f,
        &jagged,
        continents_large,
        erosion_large,
        &LARGE_BIOMES_TERRAIN,
        false,
    );
    terrain_noises(f, &jagged, continents, erosion, &AMPLIFIED_TERRAIN, true);

    let end_islands = f.define(END_ISLANDS, end_islands());
    f.define(
        SLOPED_CHEESE_END,
        end_islands + Df::reference(BASE_3D_NOISE_END),
    );
    f.define(SPAGHETTI_ROUGHNESS_FUNCTION, spaghetti_roughness());
    f.define(
        SPAGHETTI_2D_THICKNESS_MODULATOR,
        Df::mapped_noise(
            rl!("minecraft:spaghetti_2d_thickness"),
            2.0,
            1.0,
            -0.6,
            -1.3,
        )
        .cache(),
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
    names: &Terrain,
    amplified: bool,
) {
    let coordinates = Coordinates {
        continents,
        erosion,
        weirdness: Df::reference(RIDGES),
        ridges: Df::reference(RIDGES_FOLDED),
    };
    let offset = f.define(
        names.offset,
        (-0.50375 + Df::from(terrain::offset(&coordinates, amplified))).blended(Df::blend_offset()),
    );
    let factor = f.define(
        names.factor,
        Df::from(terrain::factor(&coordinates, amplified)).blended(10.0),
    );
    let depth = f.define(names.depth, offset_to_depth(&offset));
    let unscaled_jaggedness = f.define(
        names.jaggedness,
        Df::from(terrain::jaggedness(&coordinates, amplified)).blended(0.0),
    );
    let jaggedness = (unscaled_jaggedness * jagged.clone().half_negative()).cache();
    let initial_density = noise_gradient_density(&factor, depth + jaggedness);
    let sloped_cheese = f.define(
        names.sloped_cheese,
        (initial_density + Df::reference(BASE_3D_NOISE_OVERWORLD)).cache(),
    );
    let surface_level = f.define(
        names.preliminary_surface_level,
        preliminary_surface_level(&offset, &factor, amplified),
    );
    f.define(
        names.chunk_surface_level,
        surface_level.interpolated(CELL_16, CELL_1),
    );
    let surface_with_entrances = sloped_cheese.clone().min(Df::reference(ENTRANCES) * 5.0);
    let caves = Df::range_choice(
        &sloped_cheese,
        ..1.5625,
        surface_with_entrances,
        underground(&sloped_cheese),
    );
    f.define(
        names.final_density,
        post_process(slide_overworld(amplified, caves), CELL_4, CELL_8).min(Df::reference(NOODLE))
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
    let roughness = Df::noise(rl!("minecraft:spaghetti_roughness"), 1.0, 1.0);
    let modulator = Df::mapped_noise(
        rl!("minecraft:spaghetti_roughness_modulator"),
        1.0,
        1.0,
        0.0,
        -0.1,
    );
    (modulator * (roughness.abs() + -0.4)).cache()
}

fn entrances() -> Df {
    let rarity = Df::noise(rl!("minecraft:spaghetti_3d_rarity"), 2.0, 1.0).cache();
    let thickness = Df::mapped_noise(
        rl!("minecraft:spaghetti_3d_thickness"),
        1.0,
        1.0,
        -0.065,
        -0.088,
    );
    let cave = |noise: StaticResourceLocation| {
        Df::rarity_select(&rarity, noise, &[-0.5, 0.0, 0.5], &[0.75, 1.0, 1.5, 2.0])
    };
    let spaghetti_3d = (cave(rl!("minecraft:spaghetti_3d_1"))
        .max(cave(rl!("minecraft:spaghetti_3d_2")))
        + thickness)
        .clamp(-1.0, 1.0);
    let big_entrances = Df::noise(rl!("minecraft:cave_entrance"), 0.75, 0.5)
        + 0.37
        + Df::y_gradient(-10, 30, 0.3, 0.0);
    big_entrances
        .min(Df::reference(SPAGHETTI_ROUGHNESS_FUNCTION) + spaghetti_3d)
        .cache()
}

fn noodle() -> Df {
    let y = Df::reference(Y);
    let limited = |input: Df, out_of_range: f32| {
        input
            .y_limited(&y, -60..=OVERWORLD_TOP, out_of_range)
            .interpolated(CELL_4, CELL_8)
    };
    let toggle = limited(Df::noise(rl!("minecraft:noodle"), 1.0, 1.0), -1.0);
    let thickness = limited(
        Df::mapped_noise(rl!("minecraft:noodle_thickness"), 1.0, 1.0, -0.05, -0.1),
        0.0,
    );
    let ridge_frequency = 2.6666666666666665;
    let ridge = |noise: StaticResourceLocation| {
        limited(Df::noise(noise, ridge_frequency, ridge_frequency), 0.0).abs()
    };
    let ridged =
        ridge(rl!("minecraft:noodle_ridge_a")).max(ridge(rl!("minecraft:noodle_ridge_b"))) * 1.5;
    Df::range_choice(toggle, ..0.0, 64.0, thickness + ridged)
}

fn pillars() -> Df {
    let rareness = Df::mapped_noise(rl!("minecraft:pillar_rareness"), 1.0, 1.0, 0.0, -2.0);
    let thickness = Df::mapped_noise(rl!("minecraft:pillar_thickness"), 1.0, 1.0, 0.0, 1.1);
    ((Df::noise(rl!("minecraft:pillar"), 25.0, 0.3) * 2.0 + rareness) * thickness.cube()).cache()
}

fn spaghetti_2d() -> Df {
    let cave = Df::rarity_select(
        Df::noise(rl!("minecraft:spaghetti_2d_modulator"), 2.0, 1.0),
        rl!("minecraft:spaghetti_2d"),
        &[-0.75, -0.5, 0.5, 0.75],
        &[0.5, 0.75, 1.0, 2.0, 3.0],
    );
    let lowest_cell = OVERWORLD.min_y.div_euclid(8) as f32;
    let elevation = Df::mapped_noise(
        rl!("minecraft:spaghetti_2d_elevation"),
        1.0,
        0.0,
        lowest_cell,
        8.0,
    );
    let thickness = Df::reference(SPAGHETTI_2D_THICKNESS_MODULATOR);
    let sloped =
        (elevation.cache() + Df::y_gradient(OVERWORLD.min_y, OVERWORLD_TOP, 8.0, -40.0)).abs();
    let layer_ridged = (sloped + &thickness).cube();
    (cave + thickness * 0.083)
        .max(layer_ridged)
        .clamp(-1.0, 1.0)
}

fn underground(sloped_cheese: &Df) -> Df {
    let layerized_caverns = Df::noise(rl!("minecraft:cave_layer"), 1.0, 8.0).square() * 4.0;
    let cheese = Df::noise(rl!("minecraft:cave_cheese"), 1.0, 0.6666666666666666);
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

fn post_process(slide: Df, cell_size_xz: NonZeroU32, cell_size_y: NonZeroU32) -> Df {
    (slide.blend_density() * 0.64)
        .interpolated(cell_size_xz, cell_size_y)
        .squeeze()
}

pub fn full_noise(slide: Df, cell_size_xz: NonZeroU32, cell_size_y: NonZeroU32) -> Df {
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
            .interpolated(CELL_4, CELL_8)
    };
    let toggle = f.define(
        rl!("minecraft:overworld/ore_vein/toggle"),
        limited(Df::noise(rl!("minecraft:ore_veininess"), 1.5, 1.5), 0.0).cache(),
    );
    f.define(
        rl!("minecraft:overworld/ore_vein/richness"),
        toggle.clone().abs().clamped_map(0.4, 0.6, 0.1, 0.3),
    );
    let vein = |noise: StaticResourceLocation| limited(Df::noise(noise, 4.0, 4.0), 1.0).abs();
    let no_vein_near_toggle_zero = Df::range_choice(
        &toggle,
        -0.4..0.4,
        -1.0,
        0.08 - vein(rl!("minecraft:ore_vein_a")).max(vein(rl!("minecraft:ore_vein_b"))),
    );
    let mask = f.define(
        rl!("minecraft:overworld/ore_vein/mask"),
        no_vein_near_toggle_zero.cache(),
    );
    f.define(
        rl!("minecraft:overworld/ore_vein/copper_density"),
        ore_vein_density(COPPER_VEIN, &y, toggle.clone(), &mask),
    );
    f.define(
        rl!("minecraft:overworld/ore_vein/iron_density"),
        ore_vein_density(IRON_VEIN, &y, toggle.negate(), &mask),
    );
    f.define(
        rl!("minecraft:overworld/ore_vein/gap"),
        -0.3 - Df::noise(rl!("minecraft:ore_gap"), 1.0, 1.0),
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
    Df::find_top_surface(density, upper_bound, OVERWORLD.min_y, CELL_8)
}
