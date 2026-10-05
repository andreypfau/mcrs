use super::defaults::*;
use super::mob::*;
use super::*;
use mcrs_minecraft_biome::PlacedFeatureKey;
use mcrs_minecraft_keys::{ParticleType, particle_type};
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

#[derive(Serialize)]
struct AmbientSounds {
    #[serde(rename = "loop")]
    looped: &'static str,
    mood: AmbientMood,
    additions: AmbientAdditions,
}

#[derive(Serialize)]
struct AmbientMood {
    sound: &'static str,
    tick_delay: i32,
    block_search_extent: i32,
    offset: f64,
}

#[derive(Serialize)]
struct AmbientAdditions {
    sound: &'static str,
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
    kind: &'static str,
}

fn ambient_sounds(
    looped: Id<SoundEvent>,
    mood: Id<SoundEvent>,
    additions: Id<SoundEvent>,
) -> AmbientSounds {
    AmbientSounds {
        looped: looped.name(),
        mood: AmbientMood {
            sound: mood.name(),
            tick_delay: 6000,
            block_search_extent: 8,
            offset: 2.0,
        },
        additions: AmbientAdditions {
            sound: additions.name(),
            tick_chance: 0.0111,
        },
    }
}

fn ambient_particle(particle: Id<ParticleType>, probability: f32) -> AmbientParticle {
    AmbientParticle {
        particle: Particle {
            kind: particle.name(),
        },
        probability,
    }
}

macro_rules! nether_biome {
    ($music:ident, $looped:ident, $mood:ident, $additions:ident, $fog_color:expr, $mobs:expr, $generation:expr) => {
        base_biome($mobs, $generation)
            .with(FOG_COLOR, HexRgb::of($fog_color))
            .music(sound_event::$music)
            .modified(
                AMBIENT_SOUNDS,
                Operation::Override,
                ambient_sounds(
                    sound_event::$looped,
                    sound_event::$mood,
                    sound_event::$additions,
                ),
            )
    };
}

fn base_biome(mobs: Mobs, generation: Generation) -> Draft {
    Draft::new(false, 2.0, 0.0)
        .spawns(mobs.0)
        .generation(generation)
}

fn nether_generation(
    soul_fire: bool,
    patches: &[PlacedFeatureKey],
    ores: &[PlacedFeatureKey],
) -> Generation {
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .feature(VegetalDecoration, placed_feature::SPRING_LAVA)
        .features(
            UndergroundDecoration,
            &[placed_feature::SPRING_OPEN, placed_feature::PATCH_FIRE],
        );
    if soul_fire {
        g.feature(UndergroundDecoration, placed_feature::PATCH_SOUL_FIRE);
    }
    g.features(
        UndergroundDecoration,
        &[placed_feature::GLOWSTONE_EXTRA, placed_feature::GLOWSTONE],
    )
    .features(UndergroundDecoration, patches)
    .features(
        UndergroundDecoration,
        &[placed_feature::ORE_MAGMA, placed_feature::SPRING_CLOSED],
    )
    .features(UndergroundDecoration, ores);
    nether_default_ores(&mut g);
    g
}

pub fn nether_wastes() -> Draft {
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
            placed_feature::BROWN_MUSHROOM_NETHER,
            placed_feature::RED_MUSHROOM_NETHER,
        ],
        &[],
    );
    default_mushrooms(&mut g);
    nether_biome!(
        MUSIC_NETHER_NETHER_WASTES,
        AMBIENT_NETHER_WASTES_LOOP,
        AMBIENT_NETHER_WASTES_MOOD,
        AMBIENT_NETHER_WASTES_ADDITIONS,
        -13432824,
        m,
        g
    )
}

pub fn soul_sand_valley() -> Draft {
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
        &[placed_feature::PATCH_CRIMSON_ROOTS],
        &[placed_feature::ORE_SOUL_SAND],
    );
    g.feature(LocalModifications, placed_feature::BASALT_PILLAR);
    nether_biome!(
        MUSIC_NETHER_SOUL_SAND_VALLEY,
        AMBIENT_SOUL_SAND_VALLEY_LOOP,
        AMBIENT_SOUL_SAND_VALLEY_MOOD,
        AMBIENT_SOUL_SAND_VALLEY_ADDITIONS,
        -14989499,
        m,
        g
    )
    .modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(particle_type::ASH, 0.00625)],
    )
}

pub fn basalt_deltas() -> Draft {
    let mut m = Mobs::default();
    m.spawn(GHAST, 40, 1, 1)
        .spawn(MAGMA_CUBE, 100, 2, 5)
        .spawn(STRIDER, 60, 1, 2);
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .features(
            SurfaceStructures,
            &[
                placed_feature::DELTA,
                placed_feature::SMALL_BASALT_COLUMNS,
                placed_feature::LARGE_BASALT_COLUMNS,
            ],
        )
        .features(
            UndergroundDecoration,
            &[
                placed_feature::BASALT_BLOBS,
                placed_feature::BLACKSTONE_BLOBS,
                placed_feature::SPRING_DELTA,
                placed_feature::PATCH_FIRE,
                placed_feature::PATCH_SOUL_FIRE,
                placed_feature::GLOWSTONE_EXTRA,
                placed_feature::GLOWSTONE,
                placed_feature::BROWN_MUSHROOM_NETHER,
                placed_feature::RED_MUSHROOM_NETHER,
                placed_feature::ORE_MAGMA,
                placed_feature::SPRING_CLOSED_DOUBLE,
                placed_feature::ORE_GOLD_DELTAS,
                placed_feature::ORE_QUARTZ_DELTAS,
            ],
        );
    ancient_debris(&mut g);
    nether_biome!(
        MUSIC_NETHER_BASALT_DELTAS,
        AMBIENT_BASALT_DELTAS_LOOP,
        AMBIENT_BASALT_DELTAS_MOOD,
        AMBIENT_BASALT_DELTAS_ADDITIONS,
        -9937040,
        m,
        g
    )
    .modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(particle_type::WHITE_ASH, 0.118093334)],
    )
}

pub fn crimson_forest() -> Draft {
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
            placed_feature::WEEPING_VINES,
            placed_feature::CRIMSON_FUNGI,
            placed_feature::CRIMSON_FOREST_VEGETATION,
        ],
    );
    nether_biome!(
        MUSIC_NETHER_CRIMSON_FOREST,
        AMBIENT_CRIMSON_FOREST_LOOP,
        AMBIENT_CRIMSON_FOREST_MOOD,
        AMBIENT_CRIMSON_FOREST_ADDITIONS,
        -13434109,
        m,
        g
    )
    .modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(particle_type::CRIMSON_SPORE, 0.025)],
    )
}

pub fn warped_forest() -> Draft {
    let mut m = Mobs::default();
    m.spawn(ENDERMAN, 1, 4, 4)
        .spawn(STRIDER, 60, 1, 2)
        .cost(ENDERMAN, 1.0, 0.12);
    let mut g = nether_generation(true, &[], &[]);
    default_mushrooms(&mut g);
    g.features(
        VegetalDecoration,
        &[
            placed_feature::WARPED_FUNGI,
            placed_feature::WARPED_FOREST_VEGETATION,
            placed_feature::NETHER_SPROUTS,
            placed_feature::TWISTING_VINES,
        ],
    );
    nether_biome!(
        MUSIC_NETHER_WARPED_FOREST,
        AMBIENT_WARPED_FOREST_LOOP,
        AMBIENT_WARPED_FOREST_MOOD,
        AMBIENT_WARPED_FOREST_ADDITIONS,
        -15071974,
        m,
        g
    )
    .modified(
        AMBIENT_PARTICLES,
        Operation::Append,
        [ambient_particle(particle_type::WARPED_SPORE, 0.01428)],
    )
}
