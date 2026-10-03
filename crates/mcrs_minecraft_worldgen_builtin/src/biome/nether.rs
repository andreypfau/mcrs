use super::defaults::*;
use super::mob::*;
use super::*;
use crate::keys::PlacedKey;
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

#[derive(Serialize)]
struct AmbientSounds {
    #[serde(rename = "loop")]
    looped: Id,
    mood: AmbientMood,
    additions: AmbientAdditions,
}

#[derive(Serialize)]
struct AmbientMood {
    sound: Id,
    tick_delay: i32,
    block_search_extent: i32,
    offset: f64,
}

#[derive(Serialize)]
struct AmbientAdditions {
    sound: Id,
    tick_chance: f64,
}

#[derive(Serialize)]
struct AmbientParticle {
    particle: Particle,
    probability: f32,
}

#[derive(Serialize)]
struct Particle {
    #[serde(rename = "type")]
    kind: Id,
}

fn ambient_sounds(looped: Id, mood: Id, additions: Id) -> AmbientSounds {
    AmbientSounds {
        looped,
        mood: AmbientMood {
            sound: mood,
            tick_delay: 6000,
            block_search_extent: 8,
            offset: 2.0,
        },
        additions: AmbientAdditions {
            sound: additions,
            tick_chance: 0.0111,
        },
    }
}

fn ambient_particle(particle: Id, probability: f32) -> AmbientParticle {
    AmbientParticle {
        particle: Particle { kind: particle },
        probability,
    }
}

macro_rules! id {
    ($($part:expr),+) => {
        const { Id::new_static(concat!($($part),+)) }
    };
}

macro_rules! nether_biome {
    ($name:literal, $fog_color:expr, $mobs:expr, $generation:expr) => {
        base_biome($mobs, $generation)
            .with(FOG_COLOR, HexRgb::of($fog_color))
            .music(id!("minecraft:music.nether.", $name))
            .modified(
                AMBIENT_SOUNDS,
                Operation::Override,
                ambient_sounds(
                    id!("minecraft:ambient.", $name, ".loop"),
                    id!("minecraft:ambient.", $name, ".mood"),
                    id!("minecraft:ambient.", $name, ".additions"),
                ),
            )
    };
}

fn base_biome(mobs: Mobs, generation: Generation) -> Biome {
    Biome::new(false, 2.0, 0.0)
        .spawns(mobs.0)
        .generation(generation)
}

fn nether_generation(soul_fire: bool, patches: &[PlacedKey], ores: &[PlacedKey]) -> Generation {
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .feature(VegetalDecoration, placed!("spring_lava"))
        .features(
            UndergroundDecoration,
            &[placed!("spring_open"), placed!("patch_fire")],
        );
    if soul_fire {
        g.feature(UndergroundDecoration, placed!("patch_soul_fire"));
    }
    g.features(
        UndergroundDecoration,
        &[placed!("glowstone_extra"), placed!("glowstone")],
    )
    .features(UndergroundDecoration, patches)
    .features(
        UndergroundDecoration,
        &[placed!("ore_magma"), placed!("spring_closed")],
    )
    .features(UndergroundDecoration, ores);
    nether_default_ores(&mut g);
    g
}

pub fn nether_wastes() -> Biome {
    let mut m = Mobs::default();
    m.spawn(GHAST, 50, 4, 4)
        .spawn(ZOMBIFIED_PIGLIN, 100, 4, 4)
        .spawn(MAGMA_CUBE, 2, 4, 4)
        .spawn(ENDERMAN, 1, 4, 4)
        .spawn(PIGLIN, 15, 4, 4)
        .spawn(STRIDER, 60, 1, 2);
    let mut g = nether_generation(
        true,
        &[
            placed!("brown_mushroom_nether"),
            placed!("red_mushroom_nether"),
        ],
        &[],
    );
    default_mushrooms(&mut g);
    nether_biome!("nether_wastes", -13432824, m, g)
}

pub fn soul_sand_valley() -> Biome {
    let mut m = Mobs::default();
    m.spawn(SKELETON, 20, 5, 5)
        .spawn(GHAST, 50, 4, 4)
        .spawn(ENDERMAN, 1, 4, 4)
        .spawn(STRIDER, 60, 1, 2);
    for mob in [SKELETON, GHAST, ENDERMAN, STRIDER] {
        m.cost(mob, 0.7, 0.15);
    }
    let mut g = nether_generation(
        true,
        &[placed!("patch_crimson_roots")],
        &[placed!("ore_soul_sand")],
    );
    g.feature(LocalModifications, placed!("basalt_pillar"));
    nether_biome!("soul_sand_valley", -14989499, m, g).modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(rl!("minecraft:ash"), 0.00625)],
    )
}

pub fn basalt_deltas() -> Biome {
    let mut m = Mobs::default();
    m.spawn(GHAST, 40, 1, 1)
        .spawn(MAGMA_CUBE, 100, 2, 5)
        .spawn(STRIDER, 60, 1, 2);
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .features(
            SurfaceStructures,
            &[
                placed!("delta"),
                placed!("small_basalt_columns"),
                placed!("large_basalt_columns"),
            ],
        )
        .features(
            UndergroundDecoration,
            &[
                placed!("basalt_blobs"),
                placed!("blackstone_blobs"),
                placed!("spring_delta"),
                placed!("patch_fire"),
                placed!("patch_soul_fire"),
                placed!("glowstone_extra"),
                placed!("glowstone"),
                placed!("brown_mushroom_nether"),
                placed!("red_mushroom_nether"),
                placed!("ore_magma"),
                placed!("spring_closed_double"),
                placed!("ore_gold_deltas"),
                placed!("ore_quartz_deltas"),
            ],
        );
    ancient_debris(&mut g);
    nether_biome!("basalt_deltas", -9937040, m, g).modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(rl!("minecraft:white_ash"), 0.118093334)],
    )
}

pub fn crimson_forest() -> Biome {
    let mut m = Mobs::default();
    m.spawn(ZOMBIFIED_PIGLIN, 1, 2, 4)
        .spawn(HOGLIN, 9, 3, 4)
        .spawn(PIGLIN, 5, 3, 4)
        .spawn(STRIDER, 60, 1, 2);
    let mut g = nether_generation(false, &[], &[]);
    default_mushrooms(&mut g);
    g.features(
        VegetalDecoration,
        &[
            placed!("weeping_vines"),
            placed!("crimson_fungi"),
            placed!("crimson_forest_vegetation"),
        ],
    );
    nether_biome!("crimson_forest", -13434109, m, g).modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(rl!("minecraft:crimson_spore"), 0.025)],
    )
}

pub fn warped_forest() -> Biome {
    let mut m = Mobs::default();
    m.spawn(ENDERMAN, 1, 4, 4)
        .spawn(STRIDER, 60, 1, 2)
        .cost(ENDERMAN, 1.0, 0.12);
    let mut g = nether_generation(true, &[], &[]);
    default_mushrooms(&mut g);
    g.features(
        VegetalDecoration,
        &[
            placed!("warped_fungi"),
            placed!("warped_forest_vegetation"),
            placed!("nether_sprouts"),
            placed!("twisting_vines"),
        ],
    );
    nether_biome!("warped_forest", -15071974, m, g).modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(rl!("minecraft:warped_spore"), 0.01428)],
    )
}
