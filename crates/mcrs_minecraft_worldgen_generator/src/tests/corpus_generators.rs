//! Which of the corpus's features the freeze can actually run.
//!
//! A feature without a generator keeps its index and its seed and places
//! nothing, so a gap here is silent in the world and loud only in this census.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_feature::compile::CompiledPlacedFeature;
use mcrs_minecraft_worldgen_feature::proto::{Feature, Holder, PlacedFeature};

use crate::feature_program::{FeatureProgram, Generator, Nested, RunScratch};
use crate::features::FeatureTables;
use mcrs_minecraft_worldgen_feature::compile::LoadedFeatures;

use super::{
    biome_registry, block_tags, blocks, build_program, corpus_features, fluid_tags, load_json_dir,
    one_step,
};

const BIOME: &str = "minecraft:badlands";

/// The corpus features this build still cannot place.
// ponytail: `desert_well`, `sulfur_spring` and the fossils place their
// templates without the neighbour-shape pass the reference runs afterwards, so
// a sulfur spike at a template's edge keeps its file state where a real server
// may recompute its thickness; the upgrade is that post pass over the region.
const EXPECTED_MISSING: [&str; 0] = [];

/// Every feature of the corpus as one step, each placed under its own id.
fn corpus_tables() -> (FeatureTables, &'static LoadedFeatures) {
    let corpus = corpus_features();
    let step: Vec<Arc<CompiledPlacedFeature>> = corpus
        .features
        .keys()
        .map(|id| {
            Arc::new(CompiledPlacedFeature {
                id: Some(id.clone()),
                placed: PlacedFeature {
                    feature: Holder::Reference(id.clone()),
                    placement: Vec::new(),
                },
            })
        })
        .collect();
    (one_step(step, BIOME), corpus)
}

fn corpus_program() -> &'static (FeatureTables, FeatureProgram) {
    static PROGRAM: LazyLock<(FeatureTables, FeatureProgram)> = LazyLock::new(|| {
        let (tables, corpus) = corpus_tables();
        let program = build_program(&tables, corpus, &biome_registry(&[BIOME]), 0);
        (tables, program)
    });
    &PROGRAM
}

/// The step position of the corpus feature `name`, and the program's generator for it.
fn generator_of<'a>(
    tables: &FeatureTables,
    program: &'a FeatureProgram,
    name: &str,
) -> Option<&'a Generator> {
    let index = tables.features.steps[0]
        .iter()
        .position(|entry| {
            entry
                .id
                .as_ref()
                .is_some_and(|entry| entry.as_str() == name)
        })
        .unwrap_or_else(|| panic!("the corpus declares {name}"));
    program.generator_at(0, index)
}

/// The ids whose feature this build can actually write a block for.
///
/// Compiling to a generator is not enough: a container whose every entry is a
/// shape this build has no code for still compiles and still places nothing.
fn runnable() -> BTreeSet<ResourceLocation> {
    let (tables, program) = corpus_program();
    tables.features.steps[0]
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            program
                .generator_at(0, *index)
                .is_some_and(Generator::places_something)
        })
        .map(|(_, entry)| entry.id.clone().expect("every corpus entry is named"))
        .collect()
}

fn id(name: &str) -> ResourceLocation {
    ResourceLocation::parse(name).unwrap()
}

/// Which of the corpus's features still have no generator, read off the
/// registry rather than written by hand: the gap is loud here or nowhere.
#[test]
fn the_census_of_what_still_places_nothing() {
    let runnable = runnable();
    let features: BTreeMap<ResourceLocation, Feature> = load_json_dir("feature");
    let missing: BTreeSet<&str> = features
        .keys()
        .filter(|id| !runnable.contains(*id))
        .map(|id| id.as_str())
        .collect();
    let expected: BTreeSet<&str> = EXPECTED_MISSING.iter().copied().collect();
    assert_eq!(
        missing, expected,
        "the set of features with no generator moved"
    );
}

/// The sixteen selectors that reach a fallen tree kept their chance draw and
/// placed nothing for that entry; now the entry has a generator of its own.
#[test]
fn the_tree_selectors_reach_their_fallen_entry() {
    let (tables, program) = corpus_program();

    fn has_fallen(nested: &Nested) -> bool {
        matches!(nested.generator, Some(Generator::FallenTree(_)))
    }

    let Some(Generator::RandomSelector { features, default }) =
        generator_of(&tables, &program, "minecraft:trees_plains")
    else {
        panic!("trees_plains is a random_selector");
    };
    assert!(
        features.iter().any(|(_, nested)| has_fallen(nested)) || has_fallen(default),
        "no entry of trees_plains reaches fallen_oak_tree"
    );
}

/// `simple_random_selector` picks with one `nextInt(size)`, which is what a
/// weighted selector over `size` ones does — and the compile says so.
#[test]
fn a_simple_random_selector_compiles_to_equal_weights() {
    let (tables, program) = corpus_program();
    let Some(Generator::WeightedRandomSelector(features)) =
        generator_of(&tables, &program, "minecraft:forest_flowers")
    else {
        panic!("forest_flowers is a simple_random_selector");
    };
    assert!(features.iter().all(|(weight, _)| *weight == 1));
}

/// The three flower features whose block comes out of a noise sampler, and
/// which of the three shapes each takes.
#[test]
fn the_noise_state_providers_resolve_to_a_sampler() {
    use mcrs_minecraft_worldgen_feature::proto::Holder;
    use mcrs_minecraft_worldgen_feature::tree::{
        DirectBlockStateProvider, TypedBlockStateProvider,
    };
    use mcrs_minecraft_worldgen_feature_place::tree::provider::StateProvider;

    let features: BTreeMap<ResourceLocation, Feature> = load_json_dir("feature");
    let resolve = |name: &str| {
        let Some(Feature::SimpleBlock { to_place, .. }) = features.get(&id(name)) else {
            panic!("{name} is a simple_block");
        };
        to_place.clone()
    };
    for (name, matches) in [
        ("minecraft:flower_flower_forest", 0),
        ("minecraft:flower_plain", 1),
        ("minecraft:flower_meadow", 2),
    ] {
        let proto = resolve(name);
        let Holder::Reference(provider) = &proto else {
            panic!("{name} carries {proto:?}");
        };
        let shape = match corpus_features().block_state_providers.get(provider) {
            Some(DirectBlockStateProvider::Typed(TypedBlockStateProvider::Noise { .. })) => 0,
            Some(DirectBlockStateProvider::Typed(TypedBlockStateProvider::NoiseThreshold {
                ..
            })) => 1,
            Some(DirectBlockStateProvider::Typed(TypedBlockStateProvider::DualNoise {
                ..
            })) => 2,
            other => panic!("{name} names {provider}, which is {other:?}"),
        };
        assert_eq!(shape, matches, "{name}");

        let compiled = crate::trees::compile_provider(
            &proto,
            &crate::feature_program::Resolver::new(
                &blocks().0,
                Some(block_tags()),
                Some(fluid_tags()),
                &biome_registry(&[BIOME]),
                0,
                &[],
                &corpus_features().block_state_providers,
            )
            .expect("the corpus resolves"),
        )
        .unwrap_or_else(|error| panic!("{name}'s provider does not resolve: {error}"));
        assert!(
            matches!(
                compiled,
                StateProvider::Noise { .. }
                    | StateProvider::NoiseThreshold { .. }
                    | StateProvider::DualNoise { .. }
            ),
            "{name} compiled to {compiled:?}"
        );
    }
}

/// `pale_moss` rolls for a `minecraft:pale_moss_patch` on the tree's own random
/// source, so a pale oak can only run once that patch does: running it without
/// the patch's draws would move every later object of the column.
#[test]
fn a_pale_oak_runs_its_moss_patch() {
    let runnable = runnable();
    for name in [
        "minecraft:pale_moss_patch",
        "minecraft:pale_oak",
        "minecraft:pale_oak_creaking",
    ] {
        assert!(runnable.contains(&id(name)), "{name} places nothing");
    }

    let (tables, program) = corpus_program();
    assert!(
        program.run(RunScratch::default()).moss_patch.is_some(),
        "the decorator has no patch to run"
    );
    let Some(Generator::Tree(tree)) = generator_of(&tables, &program, "minecraft:pale_oak") else {
        panic!("pale_oak is a tree");
    };
    assert!(
        tree.decorators.iter().any(|decorator| matches!(
            decorator.0,
            mcrs_minecraft_worldgen_feature::tree::TreeDecorator::PaleMoss { .. }
        )),
        "pale_oak lost its pale moss decorator"
    );
}

/// Every corpus placed feature, with its own chain, as one step the single
/// biome carries — the widest program the freeze can be handed.
pub(super) fn every_placed_feature() -> FeatureTables {
    let corpus = corpus_features();
    let step = corpus
        .placed_features
        .iter()
        .map(|(id, entry)| {
            Arc::new(CompiledPlacedFeature {
                id: Some(id.clone()),
                placed: entry.clone(),
            })
        })
        .collect();
    one_step(step, "minecraft:plains")
}

/// Every feature type the corpus uses, and every feature written with it: the
/// type of each file, plus the type of every feature written inline inside one,
/// which a census over file ids cannot see.
fn corpus_feature_types() -> BTreeMap<String, Vec<Feature>> {
    fn walk(
        node: &serde_json::Value,
        is_feature: bool,
        out: &mut BTreeMap<String, Vec<serde_json::Value>>,
    ) {
        match node {
            serde_json::Value::Object(map) => {
                if is_feature && let Some(serde_json::Value::String(kind)) = map.get("type") {
                    out.entry(kind.clone()).or_default().push(node.clone());
                }
                for (key, value) in map {
                    walk(value, key == "feature", out);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, is_feature, out);
                }
            }
            _ => {}
        }
    }

    let mut raw = BTreeMap::new();
    for value in load_json_dir::<serde_json::Value>("feature").values() {
        walk(value, true, &mut raw);
    }
    raw.into_iter()
        .map(|(kind, values)| {
            let parsed = values
                .into_iter()
                .map(|value| {
                    serde_json::from_value(value)
                        .unwrap_or_else(|e| panic!("{kind} does not parse as a feature: {e}"))
                })
                .collect();
            (kind, parsed)
        })
        .collect()
}

/// The feature types no generator places.
const EXPECTED_MISSING_TYPES: [&str; 0] = [];

/// Every feature type the corpus uses, derived from the assets rather than
/// listed here, against the generators this build has. A datapack that adds a
/// type this build cannot place fails here rather than in a silent world.
#[test]
fn every_feature_type_the_corpus_uses_compiles_to_a_generator() {
    let types = corpus_feature_types();
    let mut kinds = Vec::new();
    let mut step: Vec<Arc<CompiledPlacedFeature>> = Vec::new();
    for (kind, features) in &types {
        for feature in features {
            kinds.push(kind.as_str());
            step.push(Arc::new(CompiledPlacedFeature {
                id: None,
                placed: PlacedFeature {
                    feature: Holder::Inline(Box::new(feature.clone())),
                    placement: Vec::new(),
                },
            }));
        }
    }
    let tables = one_step(step, BIOME);
    let program = build_program(&tables, corpus_features(), &biome_registry(&[BIOME]), 0);
    let covered: BTreeSet<&str> = tables.features.steps[0]
        .iter()
        .zip(&kinds)
        .enumerate()
        .filter(|(index, _)| program.generator_at(0, *index).is_some())
        .map(|(_, (_, kind))| *kind)
        .collect();
    let missing: BTreeSet<&str> = types
        .keys()
        .map(|kind| kind.as_str())
        .filter(|kind| !covered.contains(kind))
        .collect();
    assert_eq!(types.len(), 57, "the corpus's feature-type count moved");
    assert_eq!(
        missing,
        EXPECTED_MISSING_TYPES.into_iter().collect::<BTreeSet<_>>(),
        "the set of feature types with no generator moved"
    );
}

/// Every block a `simple_block` feature can hand its region, read out of the
/// asset: every name under `to_place` that the block registry resolves,
/// whatever key the provider happens to hold it under.
fn nested_simple_blocks(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    let mut out = Vec::new();
    match value {
        serde_json::Value::Object(map) => {
            if map.get("type").and_then(|kind| kind.as_str()) == Some("minecraft:simple_block") {
                out.push(value);
            }
            out.extend(map.values().flat_map(nested_simple_blocks));
        }
        serde_json::Value::Array(items) => out.extend(items.iter().flat_map(nested_simple_blocks)),
        _ => {}
    }
    out
}

fn simple_block_states(to_place: &serde_json::Value, out: &mut BTreeSet<String>) {
    match to_place {
        serde_json::Value::String(name) => {
            if blocks().0.block(name).is_some() {
                out.insert(name.clone());
            }
        }
        serde_json::Value::Object(map) => match map.get("Name").or_else(|| map.get("id")) {
            Some(serde_json::Value::String(name)) => {
                out.insert(name.clone());
            }
            _ => {
                for value in map.values() {
                    simple_block_states(value, out);
                }
            }
        },
        serde_json::Value::Array(items) => {
            for item in items {
                simple_block_states(item, out);
            }
        }
        _ => {}
    }
}

/// The pale garden's carpet is the one `simple_block` state the reference does
/// not simply write: `MossyCarpetBlock.placeAt` builds the shape from the walls
/// beside it. The compile has to hand the placer the table that lets it.
#[test]
fn the_pale_garden_carpet_compiles_to_a_shape_table() {
    let (tables, program) = corpus_program();
    let Some(Generator::SimpleBlock(config)) =
        generator_of(&tables, &program, "minecraft:pale_moss_vegetation")
    else {
        panic!("pale_moss_vegetation is a simple_block");
    };
    let carpet = config
        .tables
        .mossy_carpet
        .as_ref()
        .expect("the corpus holds pale_moss_carpet");
    assert_eq!(
        carpet.by_state.len(),
        mcrs_minecraft_worldgen_feature_place::mossy_carpet::SHAPE_COUNT,
        "every state of the block has a shape"
    );
    let moss_block = blocks()
        .0
        .block("minecraft:pale_moss_block")
        .expect("the corpus holds pale_moss_block")
        .default_state_id;
    assert!(
        carpet
            .attaches
            .iter()
            .all(|mask| mask.contains(usize::from(moss_block.0))),
        "a full cube is something the carpet can climb on every side"
    );
}

/// `SimpleBlockFeature.place` refuses a state that could not survive, so every
/// state a `simple_block` can name has to be decided: by the placement filter
/// of its definition, by a `canSurvive` family, or by the reference's default
/// of true for a block that overrides nothing.
#[test]
fn every_simple_block_state_is_decided() {
    use mcrs_minecraft_worldgen_feature_place::tree::survive::family_of;

    let mut by_filter = BTreeSet::new();
    let mut by_family = BTreeSet::new();
    let mut by_default = BTreeSet::new();
    for (_, value) in load_json_dir::<serde_json::Value>("feature") {
        let mut states = BTreeSet::new();
        for simple_block in nested_simple_blocks(&value) {
            simple_block_states(&simple_block["to_place"], &mut states);
        }
        for state in states {
            let by_definition = blocks()
                .0
                .block(&state)
                .is_some_and(|block| block.placement_filter.is_some());
            match (by_definition, family_of(&state)) {
                (true, Some(_)) => panic!("{state} is decided twice"),
                (true, None) => by_filter.insert(state),
                (false, Some(_)) => by_family.insert(state),
                (false, None) => by_default.insert(state),
            };
        }
    }
    let overrides_nothing = [
        "minecraft:basalt",
        "minecraft:brain_coral_block",
        "minecraft:bubble_coral_block",
        "minecraft:fire_coral_block",
        "minecraft:horn_coral_block",
        "minecraft:melon",
        "minecraft:potent_sulfur",
        "minecraft:pumpkin",
        "minecraft:sculk_catalyst",
        "minecraft:sculk_shrieker",
        "minecraft:tube_coral_block",
        "minecraft:tuff",
    ];
    assert_eq!(
        by_default,
        overrides_nothing.map(str::to_owned).into(),
        "a block outside every family survives anywhere, which only the blocks that override nothing do"
    );
    assert_eq!((by_filter.len(), by_family.len()), (38, 12));
}
