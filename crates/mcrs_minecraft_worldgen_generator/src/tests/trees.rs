//! The corpus's own tree features over a real overworld column: that they
//! decorate at all, and that how many they place stays where it was measured.

use mcrs_minecraft_worldgen_surface::compile::{MaterialProgram, build_router_and_material};
use std::collections::BTreeMap;
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_worldgen_feature::compile::CompiledPlacedFeature;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;

use crate::SurfaceIds;
use crate::feature_program::FeatureProgram;
use crate::features::FeatureTables;
use crate::heightmap::heightmap_predicates;
use crate::modern_carvers::ModernCarverBlockIds;
use crate::stages::{ColumnGenerator, ColumnProgram, FillContext, dimension_y_sections};

use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen_surface::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
};

use super::{
    block_tags, blocks, build_program, corpus_features, generate_region, load_json_dir, one_step,
};

/// The three biomes of the step-6 checkpoint, each with the tree feature its
/// own `worldgen/biome` file names and nothing else.
pub(super) const CHECKPOINT: [(&str, &str); 3] = [
    ("minecraft:plains", "minecraft:trees_plains"),
    (
        "minecraft:forest",
        "minecraft:trees_birch_and_oak_leaf_litter",
    ),
    ("minecraft:taiga", "minecraft:trees_taiga"),
];

/// The names `SurfaceIds::resolve` asks the registry for, which it panics
/// without, plus the three the tests decorate in.
pub(super) fn biome_registry() -> RegistrySnapshot<Biome> {
    let mut names: Vec<&str> = vec![
        "minecraft:badlands",
        "minecraft:eroded_badlands",
        "minecraft:frozen_ocean",
        "minecraft:deep_frozen_ocean",
    ];
    names.extend(CHECKPOINT.iter().map(|(biome, _)| *biome));
    names.push("minecraft:dappled_forest");
    names.push("minecraft:pale_garden");
    super::biome_registry(&names)
}

/// One step holding one placed feature, carried by the one biome — the sort
/// over the real biomes is tested elsewhere, and a band over counts does not
/// depend on the order, only on it being fixed.
pub(super) fn tree_tables(biome: &str, placed_id: &str) -> FeatureTables {
    let id = ResourceLocation::parse(placed_id).unwrap();
    let entry = corpus_features()
        .placed_features
        .get(&id)
        .cloned()
        .unwrap_or_else(|| panic!("{placed_id} is not in the corpus"));
    one_step(
        vec![Arc::new(CompiledPlacedFeature {
            id: Some(id),
            placed: entry,
        })],
        biome,
    )
}

/// The overworld router with its material rules compiled against the registry
/// below, so that the surface a tree grows on is the biome's own.
fn material_router(
    registry: &RegistrySnapshot<Biome>,
    seed: u64,
) -> (NoiseRouter, MaterialProgram) {
    let path = super::assets_root().join("noise_settings/overworld.json");
    let settings: NoiseGeneratorSettings =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = load_json_dir("material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        load_json_dir("material_condition");
    let inputs = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block: &|state| {
            blocks()
                .0
                .block(state.name.as_str())
                .map(|entry| entry.default_state_id.into())
        },
        // A biome the registry does not carry is one no rule can match, which
        // is what an id outside it means to the compiled sets.
        biome: &|id| Some(registry.by_location(id.as_str()).unwrap_or(250)),
    };
    build_router_and_material(
        &settings,
        &super::density_function_registry(),
        &super::noise_registry(),
        seed,
        super::router_blocks(&blocks().0),
        &inputs,
    )
    .expect("the overworld material rule compiles")
}

/// The overworld router, one biome everywhere, and the tree program over it.
pub(super) fn tree_dimension(biome: &str, placed_id: &str, seed: u64) -> (FillContext, Arc<[i32]>) {
    dimension_over(biome, Arc::new(tree_tables(biome, placed_id)), seed)
}

/// The overworld router and one fixed biome, over whichever tables the caller
/// wants the program built from.
pub(super) fn dimension_over(
    biome: &str,
    tables: Arc<FeatureTables>,
    seed: u64,
) -> (FillContext, Arc<[i32]>) {
    // The dispatcher builds the program from the router's own seed, and the
    // geode's noise and the End's spike ring are drawn from it; a program built
    // against a different seed is a different world.
    dimension_with(
        biome,
        |registry| build_program(&tables, corpus_features(), registry, seed as i64),
        seed,
    )
}

/// The overworld router and one fixed biome, over a program built against the
/// registry the dimension is given.
pub(super) fn dimension_with(
    biome: &str,
    program: impl FnOnce(&RegistrySnapshot<Biome>) -> FeatureProgram,
    seed: u64,
) -> (FillContext, Arc<[i32]>) {
    let registry0 = biome_registry();
    let (router, material) = material_router(&registry0, seed);
    let router = Arc::new(router);
    let y_sections = dimension_y_sections(&router, -64, 24);
    let registry = Arc::new(registry0);
    let program = program(&registry);
    let mut assets = bevy_asset::Assets::<Biome>::default();
    let handle = assets.add(super::beta_biome_palette::make_beta_biome());
    let source = Arc::new(BiomeSource::Fixed {
        biome: handle,
        biome_id: ResourceLocation::parse(biome).unwrap().to_arc(),
    });
    let ctx = FillContext {
        blocks: blocks().0.clone(),
        biome: Some((source, registry.clone())),
        predicates: Some(heightmap_predicates(blocks(), block_tags())),
        saved: None,
        program: ColumnProgram {
            generator: ColumnGenerator::Modern {
                multi_noise: None,
                surface: Some(Arc::new(SurfaceIds::resolve(&blocks().0, &registry))),
                carver_blocks: Arc::new(ModernCarverBlockIds::for_test(Vec::new())),
            },
            carvers: None,
            features: Some(Arc::new(program)),
        },
        router,
        material: Some(Arc::new(material)),
        y_sections: y_sections.clone(),
        structures: None,
    };
    (ctx, y_sections)
}

pub(super) fn tag_states(tag: &str) -> FixedBitSet {
    let mut mask = FixedBitSet::with_capacity(blocks().0.state_count());
    let key: mcrs_minecraft_core::tag_key::TagKey<
        mcrs_minecraft_block::Block,
        std::sync::Arc<str>,
    > = mcrs_minecraft_core::tag_key::TagKey::from_location(ResourceLocation::parse(tag).unwrap());
    for index in block_tags()
        .get(&key)
        .unwrap_or_else(|| panic!("{tag} is not loaded"))
        .iter()
    {
        let entry = &blocks().0.blocks()[index as usize];
        for offset in 0..entry.state_count {
            mask.insert((entry.base_state_id.0 + offset) as usize);
        }
    }
    mask
}

/// How many logs and leaves a region carries, and how many separate trunks the
/// logs form — a trunk is a run of logs with no log below it.
pub(super) struct Census {
    pub logs: usize,
    pub leaves: usize,
    pub trunks: usize,
}

pub(super) fn decorate_region(
    biome: &str,
    placed_id: &str,
    seed: u64,
    centre: ColumnPos,
    radius: i32,
) -> BTreeMap<ColumnPos, Census> {
    let (ctx, y_sections) = tree_dimension(biome, placed_id, seed);
    let logs = tag_states("minecraft:logs");
    let leaves = tag_states("minecraft:leaves");
    let region = generate_region(
        &ctx,
        ColumnPos::new(centre.x - radius, centre.z - radius),
        ColumnPos::new(centre.x + radius, centre.z + radius),
    );

    let height = y_sections.len() * 16;
    region
        .into_iter()
        .map(|(col, merged)| {
            let mut column = vec![VoxelId::default(); height * 256];
            for (slot, section) in merged.sections.iter().enumerate() {
                let Some((palette, _)) = section else {
                    continue;
                };
                for index in 0..4096 {
                    let (x, y, z) = (index & 15, index >> 8, (index >> 4) & 15);
                    column[(slot * 16 + y) * 256 + z * 16 + x] = palette.get_cell(x, y, z);
                }
            }
            let holds = |mask: &FixedBitSet, state: VoxelId| mask.contains(state.0 as usize);
            let mut census = Census {
                logs: 0,
                leaves: 0,
                trunks: 0,
            };
            for y in 0..height {
                for cell in 0..256 {
                    let state = column[y * 256 + cell];
                    if holds(&leaves, state) {
                        census.leaves += 1;
                    }
                    if !holds(&logs, state) {
                        continue;
                    }
                    census.logs += 1;
                    let below = y
                        .checked_sub(1)
                        .is_some_and(|below| holds(&logs, column[below * 256 + cell]));
                    if !below {
                        census.trunks += 1;
                    }
                }
            }
            (col, census)
        })
        .collect()
}

/// The seeds the band below was measured over, and the region each is measured
/// on: twenty-five columns around the origin, which the overworld router puts
/// on land for all four.
const SEEDS: [u64; 4] = [4242, 7331, 0xC0FFEE, 987654321];
const RADIUS: i32 = 2;

/// A regression guard: each of the three biomes decorates, and how much it
/// decorates stays where it was measured.
///
/// The band is what the first run over these seeds produced, widened. It is a
/// guard against a change that moves the picture, not a claim that the counts
/// are the reference's.
#[test]
fn plains_forest_and_taiga_decorate_within_their_measured_band() {
    // (biome, min trunks, max trunks) over one region, and the least the four
    // seeds together may come to — plains places a tree in one column in
    // twenty, so a single region of it is as readily empty as not.
    const BANDS: [(&str, usize, usize, usize); 3] = [
        ("minecraft:plains", 0, 40, 4),
        ("minecraft:forest", 20, 400, 200),
        ("minecraft:taiga", 20, 400, 200),
    ];

    for ((biome, placed), (named, low, high, least_total)) in CHECKPOINT.iter().zip(BANDS) {
        assert_eq!(*biome, named);
        let mut total = 0;
        for seed in SEEDS {
            let region = decorate_region(biome, placed, seed, ColumnPos::new(0, 0), RADIUS);
            let trunks: usize = region.values().map(|census| census.trunks).sum();
            let leaves: usize = region.values().map(|census| census.leaves).sum();
            assert_eq!(
                trunks > 0,
                leaves > 0,
                "{biome} at seed {seed}: {trunks} trunks beside {leaves} leaves"
            );
            assert!(
                (low..=high).contains(&trunks),
                "{biome} at seed {seed}: {trunks} trunks over {} columns, outside \
                 the measured band {low}..={high}",
                (2 * RADIUS + 1) * (2 * RADIUS + 1)
            );
            total += trunks;
        }
        assert!(
            total >= least_total,
            "{biome} decorated {total} trunks over the four seeds, under {least_total}"
        );
    }
}

/// `fancy_oak_bees` is the corpus's own tree with a `beehive` decorator that
/// always fires; what the corpus places it through is a one-in-a-hundred
/// rarity filter, which no region this size would reach.
fn bee_tables() -> FeatureTables {
    let mut tables = tree_tables("minecraft:plains", "minecraft:fancy_oak_bees");
    let placement: Vec<mcrs_minecraft_worldgen_feature::placement::PlacementModifier> =
        serde_json::from_str(
            r#"[{"type":"minecraft:heightmap","heightmap":"OCEAN_FLOOR"},
                {"type":"minecraft:block_predicate_filter",
                 "predicate":{"type":"minecraft:would_survive",
                              "state":"minecraft:oak_sapling"}}]"#,
        )
        .expect("the placement parses");
    let feature = tables.features.steps[0][0].placed.feature.clone();
    tables.features.steps[0][0] = Arc::new(CompiledPlacedFeature {
        id: Some(ResourceLocation::parse("minecraft:fancy_oak_bees").unwrap()),
        placed: PlacedFeature { feature, placement },
    });
    tables
}

/// The decorator's occupant draws happen because it has somewhere to write
/// them, and delivery drops what it wrote against a counter rather than
/// silently.
#[test]
fn a_generated_bee_nest_carries_its_occupants() {
    use mcrs_minecraft_worldgen_feature_place::block_entity::{
        BEE_MIN_TICKS_IN_HIVE, GeneratedBlockEntity,
    };

    let (ctx, _) = dimension_over("minecraft:plains", Arc::new(bee_tables()), 4242);

    let region = generate_region(&ctx, ColumnPos::new(-2, -2), ColumnPos::new(2, 2));
    let nests: Vec<GeneratedBlockEntity> = region
        .values()
        .flat_map(|merged| merged.block_entities.iter().cloned())
        .collect();
    assert!(
        !nests.is_empty(),
        "twenty-five columns of a tree whose beehive always fires produced no nest"
    );
    for nest in &nests {
        let GeneratedBlockEntity::Beehive { bees, .. } = nest else {
            panic!("the decorator writes a beehive");
        };
        assert!(
            (2..=3).contains(&bees.len()),
            "a nest carries {} occupants",
            bees.len()
        );
        for bee in bees {
            assert_eq!(bee.min_ticks_in_hive, BEE_MIN_TICKS_IN_HIVE);
            assert!((0..599).contains(&bee.ticks_in_hive));
        }
    }
}

/// The one tree group of the corpus that picks through a weighted list rather
/// than a chain of chances.
#[test]
fn the_weighted_selector_picks_a_tree_of_the_dappled_forest() {
    let region = decorate_region(
        "minecraft:dappled_forest",
        "minecraft:trees_dappled_forest",
        4242,
        ColumnPos::new(0, 0),
        RADIUS,
    );
    let trunks: usize = region.values().map(|census| census.trunks).sum();
    assert!(trunks > 0, "the dappled forest decorated nothing");
}

fn states_of(name: &str) -> std::ops::Range<u16> {
    let entry = blocks()
        .0
        .block(name)
        .unwrap_or_else(|| panic!("{name} is not a block"));
    let base = entry.base_state_id.0 as u16;
    base..base + entry.state_count as u16
}

/// The pale garden, decorated: pale oaks grow, and the `pale_moss` decorator's
/// own `minecraft:pale_moss_patch` runs on the tree's source, so the moss
/// ground lands with them rather than the decorator being a no-op that keeps
/// its draws and writes nothing.
#[test]
fn the_pale_garden_grows_pale_oaks_and_their_moss() {
    let (ctx, _) = tree_dimension(
        "minecraft:pale_garden",
        "minecraft:pale_garden_vegetation",
        4242,
    );
    let region = generate_region(
        &ctx,
        ColumnPos::new(-RADIUS, -RADIUS),
        ColumnPos::new(RADIUS, RADIUS),
    );
    let wanted = [
        "minecraft:pale_oak_log",
        "minecraft:pale_oak_leaves",
        "minecraft:pale_moss_block",
        "minecraft:pale_moss_carpet",
        "minecraft:short_grass",
        "minecraft:tall_grass",
    ]
    .map(|name| (name, states_of(name)));
    let mut counts = [0usize; 6];
    for merged in region.values() {
        for section in merged.sections.iter().flatten() {
            for index in 0..4096 {
                let cell = section
                    .0
                    .get_cell(index & 15, index >> 8, (index >> 4) & 15);
                for (slot, (_, states)) in wanted.iter().enumerate() {
                    counts[slot] += usize::from(states.contains(&cell.0));
                }
            }
        }
    }
    for (count, (name, _)) in counts.iter().take(3).zip(&wanted) {
        assert!(*count > 0, "the pale garden placed no {name}: {counts:?}");
    }
    for (count, (name, _)) in counts.iter().skip(3).zip(wanted.iter().skip(3)) {
        assert!(
            *count > 0,
            "the moss patch's vegetation pass placed no {name}: {counts:?}"
        );
    }
}
