//! The modern path of the `Run` stage: one ore feature, resolved at freeze and
//! placed against a region of stone.

use std::sync::Arc;

use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_worldgen_feature::compile::{CompiledPlacedFeature, FeatureSteps};
use mcrs_minecraft_worldgen_feature::placement::PlacementModifier;
use mcrs_minecraft_worldgen_feature::proto::{Feature, Holder, PlacedFeature};

use crate::ColumnBlocks;
use crate::feature_program::FeatureProgram;
use crate::features::FeatureTables;
use crate::stages::{ColumnProgram, ColumnRegion, FillContext, dimension_y_sections, run_column};
use crate::staging::RegionSnapshots;

use super::{
    TEMPERATE, bare_fill_context, biome_registry, block_tags, blocks, build_beta_router,
    build_program, flat_snapshot, fluid_tags, one_step, region_of,
};

const ORE: &str = r#"{
  "type": "minecraft:ore",
  "size": 9,
  "discard_chance_on_air_exposure": 0.0,
  "targets": [
    {
      "state": "minecraft:coal_ore",
      "target": { "predicate_type": "minecraft:block_match", "block": "minecraft:stone" }
    }
  ]
}"#;

const PLACEMENT: &str = r#"[
  { "type": "minecraft:count", "count": 20 },
  { "type": "minecraft:in_square" },
  { "type": "minecraft:height_range", "height": {
      "type": "minecraft:uniform",
      "min_inclusive": { "absolute": 8 },
      "max_inclusive": { "absolute": 60 } } },
  { "type": "minecraft:biome" }
]"#;

const BIOME: &str = "minecraft:plains";

/// One step holding one ore feature, which the one biome carries.
fn tables() -> FeatureTables {
    tables_of(ORE, PLACEMENT)
}

fn tables_of(feature_json: &str, placement_json: &str) -> FeatureTables {
    let feature: Feature = serde_json::from_str(feature_json).expect("the feature parses");
    let placement: Vec<PlacementModifier> =
        serde_json::from_str(placement_json).expect("the placement parses");
    one_step(vec![test_entry(feature, placement)], BIOME)
}

fn test_entry(feature: Feature, placement: Vec<PlacementModifier>) -> Arc<CompiledPlacedFeature> {
    Arc::new(CompiledPlacedFeature {
        id: Some(ResourceLocation::parse("minecraft:ore_coal_test").unwrap()),
        placed: PlacedFeature {
            feature: Holder::Inline(Box::new(feature)),
            placement,
        },
    })
}

fn program_of(tables: &FeatureTables, registry: &RegistrySnapshot<Biome>) -> FeatureProgram {
    build_program(tables, &Default::default(), registry, 0)
}

fn stone() -> VoxelId {
    VoxelId::from(blocks().0.default_state("minecraft:stone").0)
}

fn coal_ore() -> VoxelId {
    VoxelId::from(blocks().0.default_state("minecraft:coal_ore").0)
}

fn region(center: ColumnPos, y_sections: &Arc<[i32]>) -> RegionSnapshots {
    region_of(center, |col| {
        flat_snapshot(col, y_sections, |_| Some(stone()), Some(100))
    })
}

fn run(center: ColumnPos) -> Vec<(ColumnPos, Vec<(u32, VoxelId)>)> {
    run_tables(tables(), center)
}

fn run_tables(tables: FeatureTables, center: ColumnPos) -> Vec<(ColumnPos, Vec<(u32, VoxelId)>)> {
    let router = Arc::new(build_beta_router());
    let y_sections = dimension_y_sections(&router, -64, 24);
    let registry = biome_registry(&[BIOME]);
    let program = program_of(&tables, &registry);
    let ctx = FillContext {
        program: ColumnProgram::modern(Some(Arc::new(program))),
        ..bare_fill_context(router)
    };

    let snapshots = region(center, &y_sections);
    let column = ColumnBlocks::from_sections(&snapshots[4].sections, &y_sections);
    let mut region = ColumnRegion::new(&snapshots, &column, &ctx);
    run_column(&ctx, &mut region, 0);
    region
        .finish()
        .into_iter()
        .map(|(col, delta)| (col, delta.writes))
        .collect()
}

#[test]
fn a_modern_run_places_the_ore_the_window_biome_carries() {
    let deltas = run(ColumnPos::new(3, -5));
    let written: usize = deltas.iter().map(|(_, writes)| writes.len()).sum();
    assert_eq!(written, 128, "the twenty veins of this column");
    assert!(
        deltas.len() > 1,
        "a vein seeded at the border reaches the neighbour"
    );
    let ore = coal_ore();
    for (col, writes) in &deltas {
        for (cell, state) in writes {
            assert_eq!(*state, ore, "{col:?} took a block that is not the ore");
            assert!(*cell < 24 * ColumnBlocks::SECTION_VOLUME as u32);
        }
    }
}

/// The run is a pure function of the column, so a second run of the same
/// region writes exactly the same blocks.
#[test]
fn the_same_column_runs_to_the_same_writes() {
    let mut first = run(ColumnPos::new(3, -5));
    let mut second = run(ColumnPos::new(3, -5));
    for deltas in [&mut first, &mut second] {
        deltas.sort_by_key(|(col, _)| (col.x, col.z));
        for (_, writes) in deltas.iter_mut() {
            writes.sort();
        }
    }
    assert_eq!(first, second);
}

/// The seed is the column's block origin, so a different column decorates
/// differently.
#[test]
fn a_different_column_draws_a_different_vein() {
    let here: Vec<(u32, VoxelId)> = run(ColumnPos::new(3, -5))
        .into_iter()
        .filter(|(col, _)| *col == ColumnPos::new(3, -5))
        .flat_map(|(_, writes)| writes)
        .collect();
    let there: Vec<(u32, VoxelId)> = run(ColumnPos::new(4, -5))
        .into_iter()
        .filter(|(col, _)| *col == ColumnPos::new(4, -5))
        .flat_map(|(_, writes)| writes)
        .collect();
    assert_ne!(here, there);
}

/// A region whose palettes name no biome of the source decorates with
/// nothing, because the intersection with `possibleBiomes` is empty.
#[test]
fn a_biome_the_source_cannot_answer_with_carries_no_features() {
    let registry = biome_registry(&[BIOME]);
    let program = program_of(&tables(), &registry);
    assert!(program.slot_of(0).is_some());
    assert!(program.slot_of(1).is_none());
    assert!(
        program
            .present(&[])
            .iter()
            .all(fixedbitset::FixedBitSet::is_clear)
    );
    assert!(!program.present(&[0])[0].is_clear());
}

/// A selector naming a placed feature nothing declares is a data error, and the
/// build says which feature and which name. A shape this build has no code for
/// is not: the feature keeps its slot and places nothing.
#[test]
fn a_missing_name_fails_the_build_and_an_unsupported_shape_only_skips() {
    let registry = biome_registry(&[BIOME]);
    let selecting = tables_of(
        r#"{ "type": "minecraft:random_selector", "features": [],
             "default": "test:absent" }"#,
        "[]",
    );
    let error = FeatureProgram::build(
        &selecting,
        &Default::default(),
        &blocks().0,
        Some(block_tags()),
        Some(fluid_tags()),
        &registry,
        0,
        None,
    )
    .err()
    .expect("a missing name fails the build")
    .to_string();
    assert_eq!(
        error,
        "minecraft:ore_coal_test: unknown placed feature: test:absent"
    );

    let surviving_stone = tables_of(
        ORE,
        r#"[{ "type": "minecraft:block_predicate_filter",
              "predicate": { "type": "minecraft:would_survive",
                             "state": "minecraft:stone" } }]"#,
    );
    let program = program_of(&surviving_stone, &registry);
    assert!(
        program.generator_at(0, 0).is_none(),
        "stone has no canSurvive rule to answer the filter with"
    );
    assert!(
        program.present(&[0])[0].contains(0),
        "the biome still carries it"
    );
}

/// A nested placed feature draws from its parent's source, so the chain of one
/// whose leaf this build cannot place still has to spend its draws. Giving that
/// entry a chain must move the selector's later objects; before it ran, the two
/// programs below wrote the same blocks.
#[test]
fn a_nested_feature_without_a_generator_still_spends_its_chain() {
    fn selector(nested_chain: &str) -> String {
        format!(
            r#"{{
  "type": "minecraft:random_selector",
  "features": [
    {{
      "chance": 0.5,
      "feature": {{
        "feature": {{ "type": "minecraft:no_op" }},
        "placement": {nested_chain}
      }}
    }}
  ],
  "default": {{ "feature": {ORE}, "placement": [] }}
}}"#
        )
    }

    let center = ColumnPos::new(6, -3);
    let chained = run_tables(
        tables_of(
            &selector(
                r#"[{ "type": "minecraft:count", "count": 8 }, { "type": "minecraft:in_square" }]"#,
            ),
            PLACEMENT,
        ),
        center,
    );
    let bare = run_tables(tables_of(&selector("[]"), PLACEMENT), center);

    assert!(
        !chained.is_empty() && !bare.is_empty(),
        "the selector placed nothing either way"
    );
    assert_ne!(
        chained, bare,
        "the nested chain spent no draws on the parent's source"
    );
}

/// The reference's `biome` filter asks `BiomeGenerationSettings.hasFeature`,
/// which is a set over the biome's whole list. A placed feature two biomes name
/// at different steps therefore passes the filter at either of them.
#[test]
fn a_biome_carries_a_feature_it_names_at_any_step() {
    const OTHER: &str = "minecraft:desert";

    let registry = biome_registry(&[BIOME, OTHER]);

    // One placed feature, one `Arc`, listed by the first biome at step 0 and by
    // the second at step 1 — the shape the sort produces for a feature two
    // biomes place at different steps.
    let shared = test_entry(
        serde_json::from_str::<Feature>(ORE).expect("the ore feature parses"),
        serde_json::from_str::<Vec<PlacementModifier>>(PLACEMENT).expect("the placement parses"),
    );
    let bits = |set: bool| {
        let mut bits = fixedbitset::FixedBitSet::with_capacity(1);
        if set {
            bits.insert(0);
        }
        bits
    };
    let tables = FeatureTables {
        features: FeatureSteps {
            steps: vec![vec![shared.clone()], vec![shared]],
            per_biome: vec![vec![bits(true), bits(false)], vec![bits(false), bits(true)]],
            token: vec![vec![0], vec![0]],
        },
        biome_order: vec![
            ResourceLocation::parse(BIOME).unwrap(),
            ResourceLocation::parse(OTHER).unwrap(),
        ],
        climate: [BIOME, OTHER]
            .into_iter()
            .map(|id| (ResourceLocation::parse(id).unwrap(), TEMPERATE))
            .collect(),
    };
    let program = program_of(&tables, &registry);
    let other = registry
        .by_location(OTHER)
        .expect("the second biome is in the registry");

    assert!(
        program.carries(other, 0, 0),
        "the biome names this placed feature at step 1, so the filter passes at step 0 too"
    );
}

/// `matching_fluids` on `minecraft:empty` is true of everything holding no
/// fluid, the way the reference's fluid-less states carry `Fluids.EMPTY`. No
/// block definition interns that fluid, so matching by name alone inverted it.
#[test]
fn the_empty_fluid_matches_every_state_that_holds_no_fluid() {
    use mcrs_minecraft_core::HolderSet;
    use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, StateQuery};

    let biomes = biome_registry(&[BIOME]);
    let resolver = crate::feature_program::Resolver::new(&blocks().0, None, None, &biomes, 0, &[])
        .expect("the corpus resolves");
    let set = HolderSet::One(ResourceLocation::parse("minecraft:empty").unwrap());
    let mask = resolver
        .states(StateQuery::Fluids(&set))
        .expect("the empty fluid resolves");

    for name in ["minecraft:air", "minecraft:stone", "minecraft:short_grass"] {
        let state = blocks().0.default_state(name);
        assert!(
            mask.contains(state.0 as usize),
            "{name} holds no fluid, so the empty fluid matches it"
        );
    }
    let water = blocks().0.default_state("minecraft:water");
    assert!(
        !mask.contains(water.0 as usize),
        "water is not the empty fluid"
    );
}
