#[cfg(test)]
use mcrs_minecraft_resource::ResourceLocationBuf;
#[cfg(test)]
use mcrs_minecraft_worldgen_testing::registry;
#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
#[cfg(test)]
use std::sync::LazyLock;

#[cfg(test)]
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_worldgen_feature::compile::CompiledPlacedFeature;
#[cfg(test)]
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature};

#[cfg(test)]
use crate::feature_program::{FeatureProgram, Generator, Nested, RunScratch};
use crate::features::FeatureTables;
#[cfg(test)]
use mcrs_minecraft_worldgen_feature::compile::LoadedFeatures;

#[cfg(test)]
use super::{biome_registry, blocks, build_program};
use super::{corpus_features, one_step};

#[cfg(test)]
const BIOME: &str = "minecraft:badlands";

#[cfg(test)]
const EXPECTED_MISSING: [&str; 0] = [];

#[cfg(test)]
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

#[cfg(test)]
fn corpus_program() -> &'static (FeatureTables, FeatureProgram) {
    static PROGRAM: LazyLock<(FeatureTables, FeatureProgram)> = LazyLock::new(|| {
        let (tables, corpus) = corpus_tables();
        let program = build_program(&tables, corpus, &biome_registry(&[BIOME]), 0);
        (tables, program)
    });
    &PROGRAM
}

#[cfg(test)]
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

#[cfg(test)]
fn runnable() -> BTreeSet<ResourceLocationBuf> {
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

#[test]
fn every_feature_and_every_feature_type_of_the_corpus_has_a_generator() {
    the_census_of_what_still_places_nothing();
    every_feature_type_the_corpus_uses_compiles_to_a_generator();
}

#[test]
fn the_corpus_features_compile_to_their_shapes() {
    the_tree_selectors_reach_their_fallen_entry();
    a_simple_random_selector_compiles_to_equal_weights();
    the_noise_state_providers_resolve_to_a_sampler();
    a_pale_oak_runs_its_moss_patch();
    the_pale_garden_carpet_compiles_to_a_shape_table();
}

#[cfg(test)]
fn id(name: &str) -> ResourceLocationBuf {
    ResourceLocationBuf::read(name).unwrap()
}

#[cfg(test)]
fn the_census_of_what_still_places_nothing() {
    let runnable = runnable();
    let features: BTreeMap<ResourceLocationBuf, Feature> = registry("feature");
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

#[cfg(test)]
fn the_tree_selectors_reach_their_fallen_entry() {
    let (tables, program) = corpus_program();

    fn has_fallen(nested: &Nested) -> bool {
        matches!(nested.generator, Some(Generator::FallenTree(_)))
    }

    let Some(Generator::RandomSelector { features, default }) =
        generator_of(tables, program, "minecraft:trees_plains")
    else {
        panic!("trees_plains is a random_selector");
    };
    assert!(
        features.iter().any(|(_, nested)| has_fallen(nested)) || has_fallen(default),
        "no entry of trees_plains reaches fallen_oak_tree"
    );
}

#[cfg(test)]
fn a_simple_random_selector_compiles_to_equal_weights() {
    let (tables, program) = corpus_program();
    let Some(Generator::WeightedRandomSelector(features)) =
        generator_of(tables, program, "minecraft:forest_flowers")
    else {
        panic!("forest_flowers is a simple_random_selector");
    };
    assert!(features.iter().all(|(weight, _)| *weight == 1));
}

#[cfg(test)]
fn the_noise_state_providers_resolve_to_a_sampler() {
    use mcrs_minecraft_block_predicate::provider::{
        DirectBlockStateProvider, TypedBlockStateProvider,
    };
    use mcrs_minecraft_registries::Ref;
    use mcrs_minecraft_worldgen_feature_place::tree::provider::StateProvider;

    let features: BTreeMap<ResourceLocationBuf, Feature> = registry("feature");
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
        let Ref::Id(provider) = &proto else {
            panic!("{name} carries {proto:?}");
        };
        let shape = match corpus_features()
            .block_state_providers
            .get(provider.location())
        {
            Some(DirectBlockStateProvider::Typed(TypedBlockStateProvider::Noise { .. })) => 0,
            Some(DirectBlockStateProvider::Typed(TypedBlockStateProvider::NoiseThreshold {
                ..
            })) => 1,
            Some(DirectBlockStateProvider::Typed(TypedBlockStateProvider::DualNoise {
                ..
            })) => 2,
            other => panic!("{name} names {}, which is {other:?}", provider.location()),
        };
        assert_eq!(shape, matches, "{name}");

        let registries = biome_registry(&[BIOME]);
        let compiled = crate::trees::compile_provider(
            &proto,
            &crate::feature_program::Resolver::new(
                &blocks().0,
                &registries,
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

#[cfg(test)]
fn a_pale_oak_runs_its_moss_patch() {
    let (tables, program) = corpus_program();
    assert!(
        program.run(RunScratch::default()).moss_patch.is_some(),
        "the decorator has no patch to run"
    );
    let Some(Generator::Tree(tree)) = generator_of(tables, program, "minecraft:pale_oak") else {
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

#[cfg(test)]
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
    for value in registry::<serde_json::Value>("feature").values() {
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

#[cfg(test)]
const EXPECTED_MISSING_TYPES: [&str; 0] = [];

#[cfg(test)]
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
    assert_eq!(
        missing,
        EXPECTED_MISSING_TYPES.into_iter().collect::<BTreeSet<_>>(),
        "the set of feature types with no generator moved"
    );
}

#[cfg(test)]
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

#[cfg(test)]
fn simple_block_states(to_place: &serde_json::Value, out: &mut BTreeSet<String>) {
    match to_place {
        serde_json::Value::String(name) => {
            if blocks().0.block(name).is_some() {
                out.insert(name.clone());
            } else if let Ok(provider) = ResourceLocationBuf::read(name)
                && let Some(provider) =
                    registry::<serde_json::Value>("block_state_provider").get(&provider)
            {
                simple_block_states(provider, out);
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

#[cfg(test)]
fn the_pale_garden_carpet_compiles_to_a_shape_table() {
    let (tables, program) = corpus_program();
    let Some(Generator::SimpleBlock(config)) =
        generator_of(tables, program, "minecraft:pale_moss_vegetation")
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
            .all(|mask| mask.contains(usize::from(moss_block.number()))),
        "a full cube is something the carpet can climb on every side"
    );
}

#[test]
fn every_simple_block_state_is_decided() {
    use mcrs_minecraft_worldgen_feature_place::tree::survive::family_of;

    let mut by_default = BTreeSet::new();
    for (_, value) in registry::<serde_json::Value>("feature") {
        let mut states = BTreeSet::new();
        for simple_block in nested_simple_blocks(&value) {
            simple_block_states(&simple_block["to_place"], &mut states);
        }
        for state in states {
            let entry = blocks().0.block(&state);
            let by_definition = entry.is_some_and(|block| {
                crate::trees::short_form(&blocks().0, block.default_state_id).is_some()
            });
            let every_state = entry.is_some_and(|block| {
                (0..block.state_count)
                    .filter_map(|offset| block.state_at(offset))
                    .all(|id| crate::trees::short_form(&blocks().0, id).is_some())
            });
            match (by_definition, family_of(&state)) {
                (true, Some(_)) if every_state => panic!("{state} is decided twice"),
                (false, None) => {
                    by_default.insert(state);
                }
                _ => {}
            }
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
}
