use std::collections::HashMap;
use std::sync::Arc;

use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::climate::ParameterList;
use mcrs_minecraft_biome::overworld_preset::{nether_parameter_list, overworld_parameter_list};
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_worldgen_carver::beta::carve_beta_caves;
use mcrs_minecraft_worldgen_carver::canyon::carve_canyon;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_carver::mask::CarvingMask;
use mcrs_minecraft_worldgen_carver::modern::{SOURCE_RADIUS, carve_caves, is_start_chunk};
use mcrs_minecraft_worldgen_carver::water::WaterMask;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen_surface::compile::{MaterialProgram, build_router_and_material};
use mcrs_minecraft_worldgen_surface::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
};
use mcrs_minecraft_worldgen_testing::{registry, worldgen_dir};

use super::beta_surface::build_beta_biome_source;
use super::surface::fill_context;
use super::{build_beta_router, build_settings_router, corpus, router_blocks};
use crate::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, carving_mask, modern_carving_mask,
};
use crate::stages::{ColumnGenerator, FillContext, extent, fill_column};
use crate::task::CancellationToken;
use crate::{
    BetaCaveBlockIds, ColumnBlocks, apply_beta_surface, beta_chunk_seed, beta_surface_rng,
    generate_column,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

fn mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(FNV_PRIME)
}

fn mask_digest(mask: &CarvingMask) -> u64 {
    let mut hash = mix(
        mix(FNV_OFFSET, mask.min_y() as u32 as u64),
        mask.max_y() as u32 as u64,
    );
    mask.visit(|x, z, bottom, top| {
        for value in [x, z, bottom, top] {
            hash = mix(hash, value as u32 as u64);
        }
    });
    hash
}

#[derive(Clone, Copy)]
enum Dimension {
    Overworld,
    Nether,
}

impl Dimension {
    fn settings(self) -> &'static str {
        match self {
            Dimension::Overworld => "overworld",
            Dimension::Nether => "nether",
        }
    }

    fn preset(self) -> &'static str {
        match self {
            Dimension::Overworld => "minecraft:overworld",
            Dimension::Nether => "minecraft:nether",
        }
    }

    fn biomes(self) -> &'static ParameterList<&'static str> {
        match self {
            Dimension::Overworld => overworld_parameter_list(),
            Dimension::Nether => nether_parameter_list(),
        }
    }
}

fn carvers_of(biome: &str) -> Arc<[CarverConfig]> {
    let id = ResourceLocation::parse(biome).expect("a biome id");
    let biome: Biome = mcrs_minecraft_worldgen_testing::read("biome", &id);
    biome
        .carvers
        .iter()
        .map(|name| {
            let name = name.as_str();
            let path = worldgen_dir().join(format!(
                "carver/{}.json",
                name.strip_prefix("minecraft:").unwrap_or(name)
            ));
            serde_json::from_slice(&std::fs::read(&path).expect("the carver is shipped"))
                .expect("the carver parses")
        })
        .collect()
}

fn world(dimension: Dimension, seed: u64) -> (NoiseRouter, CarverBiomeTable) {
    (
        build_settings_router(dimension.settings(), seed),
        CarverBiomeTable::resolve(dimension.preset(), carvers_of).expect("a known preset"),
    )
}

fn square(x: std::ops::Range<i32>, z: std::ops::Range<i32>) -> Vec<(i32, i32)> {
    x.flat_map(|x| z.clone().map(move |z| (x, z))).collect()
}

fn assert_pinned<T: PartialEq + std::fmt::Debug>(got: Vec<(String, T)>, pinned: &[T]) {
    assert_eq!(got.len(), pinned.len(), "the number of pinned sets");
    let moved: Vec<String> = got
        .iter()
        .zip(pinned)
        .filter(|((_, got), pinned)| got != *pinned)
        .map(|((label, got), pinned)| format!("{label}: pinned {pinned:?}, now {got:?}"))
        .collect();
    assert!(moved.is_empty(), "{}", moved.join("\n"));
}

struct MaskSet {
    dimension: Dimension,
    seed: u64,
    columns: Vec<(i32, i32)>,
}

impl MaskSet {
    fn label(&self) -> String {
        let (first, last) = (self.columns[0], self.columns[self.columns.len() - 1]);
        format!(
            "{} seed {} columns {first:?}..{last:?}",
            self.dimension.settings(),
            self.seed
        )
    }
}

fn mask_sets() -> Vec<MaskSet> {
    let overworld = |seed, columns| MaskSet {
        dimension: Dimension::Overworld,
        seed,
        columns,
    };
    vec![
        overworld(12345, square(0..8, 0..8)),
        overworld(12345, square(-8..0, -8..0)),
        overworld(12345, square(-8..0, 0..8)),
        overworld(12345, square(8..16, -12..-4)),
        overworld(845, square(-4..4, -4..4)),
        MaskSet {
            dimension: Dimension::Nether,
            seed: 12345,
            columns: square(-4..4, -4..4),
        },
    ]
}

const MASK_DIGESTS: [u64; 6] = [
    12183299809201213350,
    11230498476353398081,
    16394469500140620552,
    5132566234595688373,
    1545640513992272029,
    17836901075532815685,
];

#[test]
fn the_per_column_mask_is_what_it_was() {
    let digests = mask_sets()
        .iter()
        .map(|set| {
            let (router, table) = world(set.dimension, set.seed);
            let height = extent(&router);
            let mut ws = Workspace::new();
            let digest = set.columns.iter().fold(FNV_OFFSET, |hash, &(x, z)| {
                let mask =
                    modern_carving_mask(x, z, set.seed as i64, &router, &mut ws, &table, height);
                mix(hash, mask_digest(&mask))
            });
            (set.label(), digest)
        })
        .collect();
    assert_pinned(digests, &MASK_DIGESTS);
}

fn material_router(
    dimension: Dimension,
    seed: u64,
    ids: &HashMap<String, u32>,
) -> (NoiseRouter, MaterialProgram) {
    let settings: NoiseGeneratorSettings = mcrs_minecraft_worldgen_testing::read(
        "noise_settings",
        &ResourceLocation::minecraft(dimension.settings()),
    );
    let rules: std::collections::BTreeMap<ResourceLocation, MaterialRuleHolder> =
        registry("material_rule");
    let conditions: std::collections::BTreeMap<ResourceLocation, MaterialConditionHolder> =
        registry("material_condition");
    let inputs = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block: &|state| {
            corpus()
                .block(state.name.as_str())
                .map(|block| block.default_state_id.into())
        },
        biome: &|id| Some(ids.get(id.as_str()).copied().unwrap_or(250)),
    };
    build_router_and_material(
        &settings,
        &registry("density_function"),
        &registry("noise"),
        seed,
        router_blocks(corpus()),
        &inputs,
    )
    .expect("the material rule compiles")
}

fn biome_registry(dimension: Dimension) -> (RegistrySnapshot<Biome>, HashMap<String, u32>) {
    let mut names: Vec<String> = Vec::new();
    let preset = dimension.biomes().values().iter().map(|(_, name)| *name);
    let surface = [
        "minecraft:eroded_badlands",
        "minecraft:frozen_ocean",
        "minecraft:deep_frozen_ocean",
    ];
    for name in preset.chain(surface) {
        if !names.iter().any(|seen| seen == name) {
            names.push(name.to_owned());
        }
    }
    let mut assets = bevy_asset::Assets::<Biome>::default();
    let pairs: Vec<_> = names
        .iter()
        .map(|name| {
            let handle = assets.add(super::beta_biome_palette::make_beta_biome());
            (
                ResourceLocation::parse(name).expect("a biome name"),
                handle.id(),
            )
        })
        .collect();
    let snapshot = RegistrySnapshot::<Biome>::build(pairs, &assets, |_| {
        Ok(mcrs_minecraft_nbt::compound::NbtCompound::new().into())
    });
    let ids = names
        .iter()
        .map(|name| {
            (
                name.clone(),
                snapshot.by_location(name).expect("registered"),
            )
        })
        .collect();
    (snapshot, ids)
}

fn carving_context(dimension: Dimension, seed: u64) -> FillContext {
    let (registry, ids) = biome_registry(dimension);
    let (router, material) = material_router(dimension, seed, &ids);
    let source = BiomeSource::MultiNoise(MultiNoiseBiomeSource {
        preset: Some(ResourceLocation::parse(dimension.preset()).expect("a preset")),
        biomes: None,
    });
    let mut context = fill_context(router, material, registry, source);
    let (_, table) = world(dimension, seed);
    context.program.carvers = Some(Arc::new(table));
    let bedrock: VoxelId = corpus().default_state("minecraft:bedrock").into();
    if let ColumnGenerator::Modern { carver_blocks, .. } = &mut context.program.generator {
        *carver_blocks = Arc::new(ModernCarverBlockIds::for_test(vec![bedrock]));
    }
    context
}

fn block_key(keys: &mut HashMap<u16, u64>, state: VoxelId) -> u64 {
    *keys.entry(state.0).or_insert_with(|| {
        let id = BlockStateId(state.0);
        let block = corpus().owner(id);
        let name = block
            .identifier
            .as_str()
            .bytes()
            .fold(FNV_OFFSET, |hash, byte| mix(hash, u64::from(byte)));
        mix(name, u64::from(id.0 - block.base_state_id.0))
    })
}

fn column_digest(context: &FillContext, column: (i32, i32), keys: &mut HashMap<u16, u64>) -> u64 {
    let mut buffer = ColumnBlocks::new(&context.y_sections);
    let snapshot = fill_column(
        context,
        ColumnPos::new(column.0, column.1),
        &mut buffer,
        &CancellationToken::new(),
    )
    .expect("the fill was not cancelled");
    let mut hash = FNV_OFFSET;
    for (index, section) in snapshot.sections.iter().enumerate() {
        hash = mix(hash, index as u64);
        let Some((blocks, _)) = section else {
            hash = mix(hash, u64::MAX);
            continue;
        };
        blocks
            .0
            .for_each(|state| hash = mix(hash, block_key(keys, state)));
    }
    hash
}

struct ColumnSet {
    dimension: Dimension,
    seed: u64,
    surfaced: bool,
    columns: Vec<(i32, i32)>,
}

impl ColumnSet {
    fn label(&self) -> String {
        format!(
            "{} seed {} {} columns {:?}",
            self.dimension.settings(),
            self.seed,
            if self.surfaced {
                "surfaced"
            } else {
                "unsurfaced"
            },
            self.columns
        )
    }
}

fn column_sets() -> Vec<ColumnSet> {
    let set = |dimension, seed, surfaced, columns: &[(i32, i32)]| ColumnSet {
        dimension,
        seed,
        surfaced,
        columns: columns.to_vec(),
    };
    let land = [
        (8, -8),
        (12, -6),
        (-20, -24),
        (-18, -24),
        (15, -24),
        (18, -12),
    ];
    let ocean = [(0, 0), (-1, 0), (0, -1), (-1, -1), (4, 4), (-8, 8)];
    let nether = [(0, 0), (-1, -1), (1, -2), (-3, 2), (8, -8), (-20, -24)];
    vec![
        set(Dimension::Overworld, 12345, true, &land),
        set(Dimension::Overworld, 845, true, &ocean),
        set(Dimension::Nether, 12345, true, &nether),
        set(Dimension::Overworld, 12345, false, &land[..3]),
        set(Dimension::Overworld, 845, false, &ocean[..3]),
        set(Dimension::Nether, 12345, false, &nether[..3]),
    ]
}

const COLUMN_DIGESTS: [u64; 6] = [
    17617883404800145981,
    2042699211622126630,
    9191105234893699405,
    1624805763646232669,
    15624454957044676763,
    11861930964516073630,
];

#[test]
fn the_carved_column_is_what_it_was() {
    let mut keys = HashMap::new();
    let digests = column_sets()
        .iter()
        .map(|set| {
            let mut context = carving_context(set.dimension, set.seed);
            if !set.surfaced {
                context.material = None;
            }
            let digest = set.columns.iter().fold(FNV_OFFSET, |hash, &column| {
                mix(hash, column_digest(&context, column, &mut keys))
            });
            (set.label(), digest)
        })
        .collect();
    assert_pinned(digests, &COLUMN_DIGESTS);
}

const CAVE: usize = 0;
const CANYON: usize = 1;
const BETA_CAVE: usize = 2;

type Counts = [[u32; 3]; 3];

fn beta_cave_count(rng: &mut LegacyRandom) -> i32 {
    let a = rng.next_i32_bound(40) + 1;
    let b = rng.next_i32_bound(a) + 1;
    let count = rng.next_i32_bound(b);
    if rng.next_i32_bound(15) != 0 {
        0
    } else {
        count
    }
}

fn tally_column(
    (chunk_x, chunk_z): (i32, i32),
    world_seed: i64,
    router: &NoiseRouter,
    table: &CarverBiomeTable,
    height: HeightContext,
    water: &WaterMask,
    counts: &mut Counts,
) {
    let mut ws = Workspace::new();
    let no_water = WaterMask::default();
    for source_x in (chunk_x - SOURCE_RADIUS)..=(chunk_x + SOURCE_RADIUS) {
        for source_z in (chunk_z - SOURCE_RADIUS)..=(chunk_z + SOURCE_RADIUS) {
            let carvers = table.carvers_of_source_for_test(router, &mut ws, source_x, source_z);
            for (index, config) in carvers.iter().enumerate() {
                let (kind, seed) = match config {
                    CarverConfig::Cave { .. } => (
                        CAVE,
                        LegacyRandom::large_feature_seed(
                            world_seed.wrapping_add(index as i64),
                            source_x,
                            source_z,
                        ),
                    ),
                    CarverConfig::Canyon { .. } => (
                        CANYON,
                        LegacyRandom::large_feature_seed(
                            world_seed.wrapping_add(index as i64),
                            source_x,
                            source_z,
                        ),
                    ),
                    CarverConfig::BetaCave => {
                        (BETA_CAVE, beta_chunk_seed(world_seed, source_x, source_z))
                    }
                };
                counts[kind][0] += 1;
                let mut rng = LegacyRandom::new(seed as u64);
                let started = match config {
                    CarverConfig::BetaCave => {
                        beta_cave_count(&mut LegacyRandom::new(seed as u64)) > 0
                    }
                    _ => is_start_chunk(config, &mut rng),
                };
                if !started {
                    continue;
                }
                counts[kind][1] += 1;
                let mut mask = carving_mask(height);
                match config {
                    CarverConfig::Cave { .. } => carve_caves(
                        config, height, chunk_x, chunk_z, source_x, source_z, &no_water, &mut mask,
                        &mut rng,
                    ),
                    CarverConfig::Canyon { .. } => carve_canyon(
                        config, height, chunk_x, chunk_z, source_x, source_z, &no_water, &mut mask,
                        &mut rng,
                    ),
                    CarverConfig::BetaCave => carve_beta_caves(
                        chunk_x, chunk_z, source_x, source_z, water, &mut mask, &mut rng,
                    ),
                }
                if !mask.is_empty() {
                    counts[kind][2] += 1;
                }
            }
        }
    }
}

fn modern_counts(dimension: Dimension, seed: u64, columns: &[(i32, i32)]) -> Counts {
    let (router, table) = world(dimension, seed);
    let height = extent(&router);
    let water = WaterMask::default();
    let mut counts = Counts::default();
    for &column in columns {
        tally_column(
            column,
            seed as i64,
            &router,
            &table,
            height,
            &water,
            &mut counts,
        );
    }
    counts
}

fn beta_water(column: &ColumnBlocks, water: VoxelId) -> WaterMask {
    let mut mask = WaterMask::default();
    for y in 0..128 {
        for x in 0..16 {
            for z in 0..16 {
                if column.get(x, y, z) == Some(water) {
                    mask.insert(x, y, z);
                }
            }
        }
    }
    mask
}

fn beta_counts(columns: &[(i32, i32)]) -> Counts {
    let router = build_beta_router();
    let (source, snapshot) = build_beta_biome_source();
    let table = super::beta_carver_table(&source);
    let ids = BetaCaveBlockIds::resolve(corpus());
    let height = extent(&router);
    let world_seed = router.world_seed as i64;
    let y_sections: Vec<i32> = (0..8).collect();
    let mut counts = Counts::default();
    for &(chunk_x, chunk_z) in columns {
        let mut sections = generate_column(
            chunk_x,
            chunk_z,
            &y_sections,
            &router,
            Some((&source, &snapshot)),
            None,
            &CancellationToken::new(),
        );
        let column = ColumnBlocks::from_sections(&sections, &y_sections);
        apply_beta_surface(
            &column,
            chunk_x * 16,
            chunk_z * 16,
            &router,
            &source,
            corpus(),
            &mut beta_surface_rng(chunk_x, chunk_z),
        );
        column.write_back(&mut sections);
        let water = beta_water(&column, ids.water);
        tally_column(
            (chunk_x, chunk_z),
            world_seed,
            &router,
            &table,
            height,
            &water,
            &mut counts,
        );
    }
    counts
}

const SOURCE_COUNTS: [Counts; 4] = [
    [[36992, 4285, 287], [18496, 272, 16], [0, 0, 0]],
    [[36992, 4760, 150], [18496, 464, 26], [0, 0, 0]],
    [[18496, 4357, 214], [0, 0, 0], [0, 0, 0]],
    [[0, 0, 0], [0, 0, 0], [18496, 730, 58]],
];

#[test]
fn the_source_counts_per_column_are_what_they_were() {
    let around = square(-4..4, -4..4);
    let counts = vec![
        (
            "overworld seed 12345".to_owned(),
            modern_counts(Dimension::Overworld, 12345, &around),
        ),
        (
            "overworld seed 845".to_owned(),
            modern_counts(Dimension::Overworld, 845, &around),
        ),
        (
            "nether seed 12345".to_owned(),
            modern_counts(Dimension::Nether, 12345, &around),
        ),
        ("beta seed 12345".to_owned(), beta_counts(&around)),
    ];
    assert_pinned(counts, &SOURCE_COUNTS);
}
