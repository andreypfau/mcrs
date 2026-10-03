use super::defaults::*;
use super::mob::*;
use super::*;
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

fn base_biome(mobs: Mobs, generation: Generation) -> Biome {
    biome(false, 2.0, 0.0, mobs, generation)
}

pub fn nether_wastes() -> Biome {
    let mut m = Mobs::default();
    m.spawn(GHAST, 50, 4, 4)
        .spawn(ZOMBIFIED_PIGLIN, 100, 4, 4)
        .spawn(MAGMA_CUBE, 2, 4, 4)
        .spawn(ENDERMAN, 1, 4, 4)
        .spawn(PIGLIN, 15, 4, 4)
        .spawn(STRIDER, 60, 1, 2);
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .feature(VegetalDecoration, placed::SPRING_LAVA);
    default_mushrooms(&mut g);
    g.features(
        UndergroundDecoration,
        &[
            placed::SPRING_OPEN,
            placed::PATCH_FIRE,
            placed::PATCH_SOUL_FIRE,
            placed::GLOWSTONE_EXTRA,
            placed::GLOWSTONE,
            placed::BROWN_MUSHROOM_NETHER,
            placed::RED_MUSHROOM_NETHER,
            placed::ORE_MAGMA,
            placed::SPRING_CLOSED,
        ],
    );
    nether_default_ores(&mut g);
    base_biome(m, g)
        .with(FOG_COLOR, HexRgb::of(-13432824))
        .music(rl!("minecraft:music.nether.nether_wastes"))
        .with(
            AMBIENT_SOUNDS,
            ambient_sounds(
                rl!("minecraft:ambient.nether_wastes.loop"),
                rl!("minecraft:ambient.nether_wastes.mood"),
                rl!("minecraft:ambient.nether_wastes.additions"),
            ),
        )
}

pub fn soul_sand_valley() -> Biome {
    let mut m = Mobs::default();
    m.spawn(SKELETON, 20, 5, 5)
        .spawn(GHAST, 50, 4, 4)
        .spawn(ENDERMAN, 1, 4, 4)
        .spawn(STRIDER, 60, 1, 2)
        .cost(SKELETON, 0.7, 0.15)
        .cost(GHAST, 0.7, 0.15)
        .cost(ENDERMAN, 0.7, 0.15)
        .cost(STRIDER, 0.7, 0.15);
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .feature(VegetalDecoration, placed::SPRING_LAVA)
        .feature(LocalModifications, placed::BASALT_PILLAR)
        .features(
            UndergroundDecoration,
            &[
                placed::SPRING_OPEN,
                placed::PATCH_FIRE,
                placed::PATCH_SOUL_FIRE,
                placed::GLOWSTONE_EXTRA,
                placed::GLOWSTONE,
                placed::PATCH_CRIMSON_ROOTS,
                placed::ORE_MAGMA,
                placed::SPRING_CLOSED,
                placed::ORE_SOUL_SAND,
            ],
        );
    nether_default_ores(&mut g);
    base_biome(m, g)
        .with(FOG_COLOR, HexRgb::of(-14989499))
        .music(rl!("minecraft:music.nether.soul_sand_valley"))
        .modified(
            AMBIENT_PARTICLES,
            Operation::Append,
            [ambient_particle(rl!("minecraft:ash"), 0.00625)],
        )
        .with(
            AMBIENT_SOUNDS,
            ambient_sounds(
                rl!("minecraft:ambient.soul_sand_valley.loop"),
                rl!("minecraft:ambient.soul_sand_valley.mood"),
                rl!("minecraft:ambient.soul_sand_valley.additions"),
            ),
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
                placed::DELTA,
                placed::SMALL_BASALT_COLUMNS,
                placed::LARGE_BASALT_COLUMNS,
            ],
        )
        .features(
            UndergroundDecoration,
            &[
                placed::BASALT_BLOBS,
                placed::BLACKSTONE_BLOBS,
                placed::SPRING_DELTA,
                placed::PATCH_FIRE,
                placed::PATCH_SOUL_FIRE,
                placed::GLOWSTONE_EXTRA,
                placed::GLOWSTONE,
                placed::BROWN_MUSHROOM_NETHER,
                placed::RED_MUSHROOM_NETHER,
                placed::ORE_MAGMA,
                placed::SPRING_CLOSED_DOUBLE,
                placed::ORE_GOLD_DELTAS,
                placed::ORE_QUARTZ_DELTAS,
            ],
        );
    ancient_debris(&mut g);
    base_biome(m, g)
        .with(FOG_COLOR, HexRgb::of(-9937040))
        .modified(
            AMBIENT_PARTICLES,
            Operation::Append,
            [ambient_particle(rl!("minecraft:white_ash"), 0.118093334)],
        )
        .music(rl!("minecraft:music.nether.basalt_deltas"))
        .with(
            AMBIENT_SOUNDS,
            ambient_sounds(
                rl!("minecraft:ambient.basalt_deltas.loop"),
                rl!("minecraft:ambient.basalt_deltas.mood"),
                rl!("minecraft:ambient.basalt_deltas.additions"),
            ),
        )
}

pub fn crimson_forest() -> Biome {
    let mut m = Mobs::default();
    m.spawn(ZOMBIFIED_PIGLIN, 1, 2, 4)
        .spawn(HOGLIN, 9, 3, 4)
        .spawn(PIGLIN, 5, 3, 4)
        .spawn(STRIDER, 60, 1, 2);
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .feature(VegetalDecoration, placed::SPRING_LAVA);
    default_mushrooms(&mut g);
    g.features(
        UndergroundDecoration,
        &[
            placed::SPRING_OPEN,
            placed::PATCH_FIRE,
            placed::GLOWSTONE_EXTRA,
            placed::GLOWSTONE,
            placed::ORE_MAGMA,
            placed::SPRING_CLOSED,
        ],
    )
    .features(
        VegetalDecoration,
        &[
            placed::WEEPING_VINES,
            placed::CRIMSON_FUNGI,
            placed::CRIMSON_FOREST_VEGETATION,
        ],
    );
    nether_default_ores(&mut g);
    base_biome(m, g)
        .with(FOG_COLOR, HexRgb::of(-13434109))
        .music(rl!("minecraft:music.nether.crimson_forest"))
        .modified(
            AMBIENT_PARTICLES,
            Operation::Append,
            [ambient_particle(rl!("minecraft:crimson_spore"), 0.025)],
        )
        .with(
            AMBIENT_SOUNDS,
            ambient_sounds(
                rl!("minecraft:ambient.crimson_forest.loop"),
                rl!("minecraft:ambient.crimson_forest.mood"),
                rl!("minecraft:ambient.crimson_forest.additions"),
            ),
        )
}

pub fn warped_forest() -> Biome {
    let mut m = Mobs::default();
    m.spawn(ENDERMAN, 1, 4, 4)
        .spawn(STRIDER, 60, 1, 2)
        .cost(ENDERMAN, 1.0, 0.12);
    let mut g = Generation::default();
    g.carver(carver::NETHER_CAVE)
        .feature(VegetalDecoration, placed::SPRING_LAVA);
    default_mushrooms(&mut g);
    g.features(
        UndergroundDecoration,
        &[
            placed::SPRING_OPEN,
            placed::PATCH_FIRE,
            placed::PATCH_SOUL_FIRE,
            placed::GLOWSTONE_EXTRA,
            placed::GLOWSTONE,
            placed::ORE_MAGMA,
            placed::SPRING_CLOSED,
        ],
    )
    .features(
        VegetalDecoration,
        &[
            placed::WARPED_FUNGI,
            placed::WARPED_FOREST_VEGETATION,
            placed::NETHER_SPROUTS,
            placed::TWISTING_VINES,
        ],
    );
    nether_default_ores(&mut g);
    base_biome(m, g)
        .with(FOG_COLOR, HexRgb::of(-15071974))
        .music(rl!("minecraft:music.nether.warped_forest"))
        .modified(
            AMBIENT_PARTICLES,
            Operation::Append,
            [ambient_particle(rl!("minecraft:warped_spore"), 0.01428)],
        )
        .with(
            AMBIENT_SOUNDS,
            ambient_sounds(
                rl!("minecraft:ambient.warped_forest.loop"),
                rl!("minecraft:ambient.warped_forest.mood"),
                rl!("minecraft:ambient.warped_forest.additions"),
            ),
        )
}
