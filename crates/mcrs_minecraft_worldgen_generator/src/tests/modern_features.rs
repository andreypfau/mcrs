//! The modern path of the `Run` stage: one ore feature, resolved at freeze and
//! placed against a region of stone.

use mcrs_minecraft_core::ResourceKey;
use std::sync::Arc;

use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{KeySet, Registries};
use mcrs_minecraft_worldgen_feature::compile::{CompiledPlacedFeature, FeatureSteps};
use mcrs_minecraft_worldgen_feature::placement::PlacementModifier;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature};
use mcrs_voxel::ColumnPos;

use crate::ColumnBlocks;
use crate::feature_program::FeatureProgram;
use crate::features::FeatureTables;
use crate::stages::{ColumnProgram, ColumnRegion, FillContext, dimension_y_sections, run_column};
use crate::staging::RegionSnapshots;

use super::{
    TEMPERATE, bare_fill_context, biome_registry, blocks, build_beta_router, build_program,
    flat_snapshot, one_step, region_of, try_build_program,
};
use mcrs_minecraft_block::keys::Fluid;

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
        id: Some(ResourceLocation::read("minecraft:ore_coal_test").unwrap()),
        placed: PlacedFeature {
            feature: Holder::Inline(Box::new(feature)),
            placement,
        },
    })
}

fn program_of(tables: &FeatureTables, registry: &Registries) -> FeatureProgram {
    build_program(tables, &Default::default(), registry, 0)
}

fn stone() -> VoxelId {
    VoxelId::from(
        blocks()
            .0
            .default_state("minecraft:stone")
            .unwrap()
            .number(),
    )
}

fn coal_ore() -> VoxelId {
    VoxelId::from(
        blocks()
            .0
            .default_state("minecraft:coal_ore")
            .unwrap()
            .number(),
    )
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
    let error = try_build_program(&selecting, &Default::default(), &registry, 0, None)
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
            ResourceLocation::read(BIOME).unwrap(),
            ResourceLocation::read(OTHER).unwrap(),
        ],
        climate: [BIOME, OTHER]
            .into_iter()
            .map(|id| (ResourceLocation::read(id).unwrap(), TEMPERATE))
            .collect(),
    };
    let program = program_of(&tables, &registry);
    let other = super::biome_id(&registry, OTHER).number();

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
    use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, StateQuery};

    let registries = biome_registry(&[BIOME]);
    let resolver = crate::feature_program::Resolver::new(
        &blocks().0,
        &registries,
        0,
        &[],
        &super::corpus_features().block_state_providers,
    )
    .expect("the corpus resolves");
    let set = fluid_set("minecraft:empty");
    let mask = resolver
        .states(StateQuery::Fluids(
            &set,
            &mcrs_minecraft_block_predicate::state_properties::StatePropertiesPredicate::ANY,
        ))
        .expect("the empty fluid resolves");

    for name in ["minecraft:air", "minecraft:stone", "minecraft:short_grass"] {
        let state = blocks().0.default_state(name).unwrap();
        assert!(
            mask.contains(state.number() as usize),
            "{name} holds no fluid, so the empty fluid matches it"
        );
    }
    let water = blocks().0.default_state("minecraft:water").unwrap();
    assert!(
        !mask.contains(water.number() as usize),
        "water is not the empty fluid"
    );
}

#[test]
fn a_spring_places_its_fluid_state_as_the_fluid_blocks_level() {
    use mcrs_minecraft_block_predicate::fluid_state::FluidState;

    fluid_resolver_test(|resolver| {
        for (json, block, level) in [
            (
                r#"{"id":"minecraft:water","properties":{"level":"8","source":"true"}}"#,
                "water",
                "0",
            ),
            (
                r#"{"id":"minecraft:water","properties":{"level":"3","source":"true"}}"#,
                "water",
                "0",
            ),
            (
                r#"{"id":"minecraft:lava","properties":{"level":"3","source":"false"}}"#,
                "lava",
                "5",
            ),
            (
                r#"{"id":"minecraft:water","properties":{"level":"8","source":"false"}}"#,
                "water",
                "8",
            ),
        ] {
            let state: FluidState = serde_json::from_str(json).expect("a fluid state");
            assert_eq!(
                crate::feature_program::spring_state(&state, resolver),
                Some(corpus_state(resolver, block, &[("level", level)])),
                "{json}"
            );
        }
    });
}

fn fluid_resolver_test<F: FnOnce(&crate::feature_program::Resolver<'_>)>(test: F) {
    let registries = biome_registry(&[BIOME]);
    let providers = std::collections::BTreeMap::new();
    let resolver =
        crate::feature_program::Resolver::new(&blocks().0, &registries, 0, &[], &providers)
            .expect("the corpus resolves");
    test(&resolver);
}

fn corpus_state(
    resolver: &crate::feature_program::Resolver<'_>,
    name: &str,
    properties: &[(&str, &str)],
) -> VoxelId {
    let state = properties.iter().fold(
        mcrs_minecraft_block_predicate::block_state::BlockState::minecraft(name).unwrap(),
        |state, (property, value)| state.with(property, value),
    );
    resolver.resolve(&state).expect("a corpus block state")
}

#[test]
fn a_source_only_fluid_query_holds_sources_and_waterlogged_blocks() {
    use mcrs_minecraft_block_predicate::state_properties::StatePropertiesPredicate;
    use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, StateQuery};

    fluid_resolver_test(|resolver| {
        let sources: StatePropertiesPredicate =
            serde_json::from_str(r#"{"source": "true"}"#).unwrap();
        let mask = resolver
            .states(StateQuery::Fluids(&fluid_set("minecraft:water"), &sources))
            .expect("water resolves");
        for (name, properties, held) in [
            ("water", &[("level", "0")][..], true),
            ("oak_stairs", &[("waterlogged", "true")][..], true),
            ("water", &[("level", "1")][..], false),
            ("water", &[("level", "8")][..], false),
        ] {
            let state = corpus_state(resolver, name, properties);
            assert_eq!(
                mask.contains(usize::from(state.0)),
                held,
                "{name} {properties:?}"
            );
        }
    });
}

#[test]
fn a_survival_rule_reads_the_water_condition_its_block_names() {
    use mcrs_minecraft_block::keys::Block;
    use mcrs_voxel::BlockPos;

    fluid_resolver_test(|resolver| {
        let rules = crate::trees::survive_rules(resolver).expect("the survival rules compile");
        let water = |level: &str| corpus_state(resolver, "water", &[("level", level)]);
        let air = corpus_state(resolver, "air", &[]);
        let stone = corpus_state(resolver, "stone", &[]);
        let dirt = corpus_state(resolver, "dirt", &[]);
        let sand = corpus_state(resolver, "sand", &[]);
        for (block, below, at, above, survives) in [
            (Block::LilyPad, water("0"), air, air, true),
            (Block::LilyPad, water("1"), air, air, false),
            (Block::SporeBlossom, air, air, stone, true),
            (Block::SporeBlossom, air, water("1"), stone, false),
            (Block::SmallDripleaf, dirt, water("0"), air, true),
            (Block::SmallDripleaf, dirt, water("1"), air, false),
            (Block::TallSeagrass, sand, water("8"), water("0"), true),
            (Block::TallSeagrass, sand, water("5"), water("0"), false),
        ] {
            let rule = rules
                .get(&block.id())
                .expect("the block has a survival rule");
            let origin = BlockPos::new(0, 64, 0);
            let survived = rule.test(air, origin, |pos| match pos.y - origin.y {
                -1 if pos.x == 0 && pos.z == 0 => below,
                0 if pos.x == 0 && pos.z == 0 => at,
                1 if pos.x == 0 && pos.z == 0 => above,
                _ => air,
            });
            assert_eq!(
                survived, survives,
                "{block:?} over {below:?} in {at:?} under {above:?}"
            );
        }
    });
}

fn fluid_set(name: &str) -> KeySet<Fluid> {
    KeySet::List(Box::new([ResourceKey::from_location(
        ResourceLocation::read(name).unwrap(),
    )]))
}
