use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::ResourceLocation;

use mcrs_minecraft_chunk::VoxelId;

use bevy_math::IVec3;

use super::block_predicate::{BlockPredicate, Direction, Offset};
use super::placement::PlacementModifier;
use super::placer::{BiomeMask, Modifier, Predicate, Rule, StateMask, single_state};
use super::proto::{Feature, FeatureStepList, Holder, PlacedFeature, StructureProcessorList};
use super::rule_test::RuleTest;
use super::sort::build_features_per_step;
use crate::template::Template;
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_worldgen_density::proto::BlockState;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FeatureCompileError {
    #[error("unknown feature: {0}")]
    UnknownFeature(ResourceLocation),
    #[error("unknown placed feature: {0}")]
    UnknownPlacedFeature(ResourceLocation),
    #[error("unknown placed feature tag: {0}")]
    UnknownTag(ResourceLocation),
    #[error("unknown template: {0}")]
    UnknownTemplate(ResourceLocation),
    #[error("unknown processor list: {0}")]
    UnknownProcessorList(ResourceLocation),
    #[error("template {0}")]
    Template(String),
    #[error("feature order cycle through {0}")]
    FeatureCycle(String),
    #[error("unknown block state: {0}")]
    UnknownBlockState(String),
    #[error("unknown block set: {0}")]
    UnknownBlockSet(String),
    #[error("unknown biome set: {0}")]
    UnknownBiomeSet(String),
    /// A shape this build has no code for yet. Unlike every other variant it is
    /// not a data error: the feature keeps its slot and places nothing.
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("{0}: {1}")]
    In(String, Box<FeatureCompileError>),
}

impl FeatureCompileError {
    pub fn within(self, what: impl std::fmt::Display) -> Self {
        FeatureCompileError::In(what.to_string(), Box::new(self))
    }

    pub fn is_unsupported(&self) -> bool {
        match self {
            FeatureCompileError::Unsupported(_) => true,
            FeatureCompileError::In(_, inner) => inner.is_unsupported(),
            _ => false,
        }
    }
}

/// Both feature registries as they were loaded, shared by every dimension,
/// with the templates and processor lists the features name.
#[derive(Default)]
pub struct LoadedFeatures {
    pub features: BTreeMap<ResourceLocation, Feature>,
    pub placed_features: BTreeMap<ResourceLocation, PlacedFeature>,
    pub templates: BTreeMap<ResourceLocation, Template>,
    pub processor_lists: BTreeMap<ResourceLocation, StructureProcessorList>,
}

/// One vertex of the sorted order: a placed feature and, unless it was written
/// inline, the id it was named by.
#[derive(Debug)]
pub struct CompiledPlacedFeature {
    pub id: Option<ResourceLocation>,
    pub placed: PlacedFeature,
}

/// What a dimension's generation runs, per decoration step.
#[derive(Debug, Clone)]
pub struct FeatureSteps {
    /// A feature's position in its step's list is the index of its seed.
    pub steps: Vec<Vec<Arc<CompiledPlacedFeature>>>,
    /// `[biome][step]`, a bit per entry of `steps[step]`.
    pub per_biome: Vec<Vec<FixedBitSet>>,
    /// Index-aligned with `steps`: the identity of each entry, shared by every
    /// step and biome that names the same placed feature.
    pub token: Vec<Vec<usize>>,
}

/// `biomes` is the source's list in its own order, each with the steps it
/// declares.
pub fn build_feature_steps(
    biomes: &[&[FeatureStepList]],
    registries: &LoadedFeatures,
) -> Result<FeatureSteps, FeatureCompileError> {
    let mut interner = Interner::new(registries);
    let mut tokens_per_biome: Vec<Vec<Vec<usize>>> = Vec::with_capacity(biomes.len());

    for biome in biomes {
        let mut steps = Vec::with_capacity(biome.len());
        for list in *biome {
            let mut tokens = Vec::new();
            if let HolderSet::Tag(tag) = list {
                return Err(FeatureCompileError::UnknownTag(tag.clone()));
            }
            for holder in list.entries() {
                tokens.push(interner.token(holder)?);
            }
            steps.push(tokens);
        }
        tokens_per_biome.push(steps);
    }

    let sorted = build_features_per_step(&tokens_per_biome).map_err(|token| {
        FeatureCompileError::FeatureCycle(match &interner.entries[token].id {
            Some(id) => id.to_string(),
            None => "an inline feature".to_owned(),
        })
    })?;

    let position: Vec<HashMap<usize, usize>> = sorted
        .iter()
        .map(|step| {
            step.iter()
                .enumerate()
                .map(|(position, &token)| (token, position))
                .collect()
        })
        .collect();

    let per_biome = tokens_per_biome
        .iter()
        .map(|biome| {
            sorted
                .iter()
                .enumerate()
                .map(|(step, features)| {
                    let mut bits = FixedBitSet::with_capacity(features.len());
                    for token in biome.get(step).into_iter().flatten() {
                        bits.insert(position[step][token]);
                    }
                    bits
                })
                .collect()
        })
        .collect();

    let steps = sorted
        .iter()
        .map(|step| {
            step.iter()
                .map(|&token| interner.entries[token].clone())
                .collect()
        })
        .collect();

    Ok(FeatureSteps {
        steps,
        per_biome,
        token: sorted,
    })
}

/// Gives every distinct placed feature one identity: registry entries by id, so
/// two biomes naming one id share a vertex, and every inline entry its own.
struct Interner<'a> {
    registries: &'a LoadedFeatures,
    entries: Vec<Arc<CompiledPlacedFeature>>,
    by_id: HashMap<ResourceLocation, usize>,
}

impl<'a> Interner<'a> {
    fn new(registries: &'a LoadedFeatures) -> Self {
        Interner {
            registries,
            entries: Vec::new(),
            by_id: HashMap::new(),
        }
    }

    fn token(&mut self, holder: &Holder<PlacedFeature>) -> Result<usize, FeatureCompileError> {
        match holder {
            Holder::Reference(id) => self.reference(id),
            Holder::Inline(placed) => Ok(self.push(None, placed)),
        }
    }

    fn reference(&mut self, id: &ResourceLocation) -> Result<usize, FeatureCompileError> {
        if let Some(&token) = self.by_id.get(id) {
            return Ok(token);
        }
        let placed = self
            .registries
            .placed_features
            .get(id)
            .ok_or_else(|| FeatureCompileError::UnknownPlacedFeature(id.clone()))?;
        let token = self.push(Some(id.clone()), placed);
        self.by_id.insert(id.clone(), token);
        Ok(token)
    }

    fn push(&mut self, id: Option<ResourceLocation>, placed: &PlacedFeature) -> usize {
        self.entries.push(Arc::new(CompiledPlacedFeature {
            id,
            placed: placed.clone(),
        }));
        self.entries.len() - 1
    }
}

/// A set of block states the world names, that the run resolves to numbers.
///
/// Everything a `BlockPredicate` or a `RuleTest` asks about a state is a pure
/// function of the state, so each of these collapses to one mask at freeze and
/// nothing looks a name up again while a column generates.
pub enum StateQuery<'a> {
    Blocks(&'a HolderSet),
    Block(&'a ResourceLocation),
    BlockTag(&'a ResourceLocation),
    Fluids(&'a HolderSet),
    SturdyFace(Direction),
    Solid,
    Replaceable,
}

/// The block and biome registries as the feature compiler needs them.
pub trait BlockResolver {
    fn state(&self, state: &BlockState) -> Option<VoxelId>;

    fn states(&self, query: StateQuery<'_>) -> Option<StateMask>;

    fn biomes(&self, set: &HolderSet) -> Option<BiomeMask>;
}

pub fn named(set: &HolderSet) -> String {
    match set {
        HolderSet::Tag(tag) => format!("#{tag}"),
        HolderSet::One(id) => id.to_string(),
        HolderSet::List(ids) => ids
            .iter()
            .map(ResourceLocation::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

pub fn state_named(state: &BlockState) -> String {
    match &state.properties {
        None => state.name.to_string(),
        Some(properties) => format!(
            "{}[{}]",
            state.name,
            properties
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

impl std::fmt::Display for StateQuery<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateQuery::Blocks(set) | StateQuery::Fluids(set) => f.write_str(&named(set)),
            StateQuery::Block(id) => write!(f, "{id}"),
            StateQuery::BlockTag(tag) => write!(f, "#{tag}"),
            StateQuery::SturdyFace(direction) => write!(f, "sturdy face {direction:?}"),
            StateQuery::Solid => f.write_str("solid"),
            StateQuery::Replaceable => f.write_str("replaceable"),
        }
    }
}

pub fn states_of(
    blocks: &dyn BlockResolver,
    query: StateQuery<'_>,
) -> Result<StateMask, FeatureCompileError> {
    let name = query.to_string();
    blocks
        .states(query)
        .ok_or(FeatureCompileError::UnknownBlockSet(name))
}

pub fn state_of(
    blocks: &dyn BlockResolver,
    state: &BlockState,
) -> Result<VoxelId, FeatureCompileError> {
    blocks
        .state(state)
        .ok_or_else(|| FeatureCompileError::UnknownBlockState(state_named(state)))
}

pub fn compile_predicate(
    predicate: &BlockPredicate,
    blocks: &dyn BlockResolver,
) -> Result<Predicate, FeatureCompileError> {
    let matching = |offset: &Offset, query| {
        Ok::<_, FeatureCompileError>(Predicate::MatchingStates {
            offset: IVec3::from_array(offset.0),
            states: states_of(blocks, query)?,
        })
    };
    Ok(match predicate {
        BlockPredicate::True => Predicate::True,
        BlockPredicate::MatchingBlocks {
            offset,
            blocks: set,
        } => matching(offset, StateQuery::Blocks(set))?,
        BlockPredicate::MatchingBlockTag { offset, tag } => {
            matching(offset, StateQuery::BlockTag(tag))?
        }
        BlockPredicate::MatchingFluids { offset, fluids } => {
            matching(offset, StateQuery::Fluids(fluids))?
        }
        BlockPredicate::HasSturdyFace { offset, direction } => {
            matching(offset, StateQuery::SturdyFace(*direction))?
        }
        BlockPredicate::Solid { offset } => matching(offset, StateQuery::Solid)?,
        BlockPredicate::Replaceable { offset } => matching(offset, StateQuery::Replaceable)?,
        BlockPredicate::MatchingBiomes { biomes } => Predicate::MatchingBiomes(
            blocks
                .biomes(biomes)
                .ok_or_else(|| FeatureCompileError::UnknownBiomeSet(named(biomes)))?,
        ),
        BlockPredicate::WouldSurvive { offset, state } => Predicate::WouldSurvive {
            offset: IVec3::from_array(offset.0),
            state: state_of(blocks, state)?,
        },
        BlockPredicate::InsideWorldBounds { offset } => Predicate::InsideWorldBounds {
            offset_y: offset.0[1],
        },
        BlockPredicate::HeightRange {
            min_inclusive,
            max_inclusive,
        } => Predicate::HeightRange {
            min_inclusive: *min_inclusive,
            max_inclusive: *max_inclusive,
        },
        BlockPredicate::VolumeMatch(volume) => Predicate::VolumeMatch {
            min: volume.min.0,
            max: volume.max.0,
            matches: Box::new(compile_predicate(&volume.r#match, blocks)?),
        },
        BlockPredicate::AnyOf { predicates } => {
            Predicate::AnyOf(compile_predicates(predicates, blocks)?)
        }
        BlockPredicate::AllOf { predicates } => {
            Predicate::AllOf(compile_predicates(predicates, blocks)?)
        }
        BlockPredicate::Not { predicate } => {
            Predicate::Not(Box::new(compile_predicate(predicate, blocks)?))
        }
        BlockPredicate::Unobstructed { .. } => {
            return Err(FeatureCompileError::Unsupported(
                "minecraft:unobstructed".to_owned(),
            ));
        }
    })
}

fn compile_predicates(
    predicates: &[BlockPredicate],
    blocks: &dyn BlockResolver,
) -> Result<Vec<Predicate>, FeatureCompileError> {
    predicates
        .iter()
        .map(|predicate| compile_predicate(predicate, blocks))
        .collect()
}

pub fn compile_rule(
    rule: &RuleTest,
    blocks: &dyn BlockResolver,
) -> Result<Rule, FeatureCompileError> {
    Ok(match rule {
        RuleTest::AlwaysTrue => Rule::AlwaysTrue,
        RuleTest::BlockMatch { block } => {
            Rule::MatchingStates(states_of(blocks, StateQuery::Block(block))?)
        }
        RuleTest::BlockStateMatch { block_state } => {
            Rule::MatchingStates(single_state(state_of(blocks, block_state)?))
        }
        RuleTest::TagMatch { tag } => {
            Rule::MatchingStates(states_of(blocks, StateQuery::BlockTag(tag))?)
        }
        RuleTest::HeightMatch {
            min_inclusive,
            max_inclusive,
        } => Rule::HeightMatch {
            min_inclusive: *min_inclusive,
            max_inclusive: *max_inclusive,
        },
        RuleTest::RandomBlockMatch { block, probability } => Rule::RandomStates {
            states: states_of(blocks, StateQuery::Block(block))?,
            probability: *probability,
        },
        RuleTest::RandomBlockStateMatch {
            block_state,
            probability,
        } => Rule::RandomStates {
            states: single_state(state_of(blocks, block_state)?),
            probability: *probability,
        },
        RuleTest::AllOf { rules } => Rule::AllOf(compile_rules(rules, blocks)?),
        RuleTest::AnyOf { rules } => Rule::AnyOf(compile_rules(rules, blocks)?),
        RuleTest::Not { rule } => Rule::Not(Box::new(compile_rule(rule, blocks)?)),
    })
}

fn compile_rules(
    rules: &[RuleTest],
    blocks: &dyn BlockResolver,
) -> Result<Vec<Rule>, FeatureCompileError> {
    rules
        .iter()
        .map(|rule| compile_rule(rule, blocks))
        .collect()
}

pub fn compile_placement(
    placement: &[PlacementModifier],
    blocks: &dyn BlockResolver,
) -> Result<Vec<Modifier>, FeatureCompileError> {
    placement
        .iter()
        .map(|modifier| modifier.try_map(&mut |predicate| compile_predicate(predicate, blocks)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::UnitFloat;

    fn id(name: &str) -> ResourceLocation {
        ResourceLocation::parse(name).unwrap()
    }

    fn bamboo() -> Feature {
        Feature::Bamboo {
            probability: serde_json::from_str::<UnitFloat>("0.0").unwrap(),
        }
    }

    fn placed(feature: &str) -> PlacedFeature {
        PlacedFeature {
            feature: Holder::Reference(id(feature)),
            placement: Vec::new(),
        }
    }

    fn corpus() -> LoadedFeatures {
        let mut corpus = LoadedFeatures::default();
        corpus.features.insert(id("test:leaf"), bamboo());
        for name in ["test:a", "test:b", "test:c"] {
            corpus.placed_features.insert(id(name), placed("test:leaf"));
        }
        corpus
    }

    fn steps(entries: &[&str]) -> Vec<FeatureStepList> {
        vec![FeatureStepList::List(
            entries
                .iter()
                .map(|name| Holder::Reference(id(name)))
                .collect(),
        )]
    }

    fn ids(steps: &FeatureSteps) -> Vec<Vec<Option<String>>> {
        steps
            .steps
            .iter()
            .map(|step| {
                step.iter()
                    .map(|feature| feature.id.as_ref().map(|id| id.to_string()))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn two_biomes_naming_one_id_share_a_vertex() {
        let corpus = corpus();
        let first = steps(&["test:a", "test:b"]);
        let second = steps(&["test:b", "test:c"]);
        let built = build_feature_steps(&[&first, &second], &corpus).unwrap();

        assert_eq!(
            ids(&built),
            vec![vec![
                Some("test:a".to_owned()),
                Some("test:b".to_owned()),
                Some("test:c".to_owned())
            ]]
        );
        assert_eq!(built.per_biome[0][0].ones().collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(built.per_biome[1][0].ones().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn an_inline_entry_is_an_object_of_its_own() {
        let corpus = corpus();
        let inline = || {
            vec![FeatureStepList::List(vec![
                Holder::Inline(Box::new(placed("test:leaf"))),
                Holder::Reference(id("test:a")),
            ])]
        };
        let first = inline();
        let second = inline();
        let built = build_feature_steps(&[&first, &second], &corpus).unwrap();

        assert_eq!(
            ids(&built),
            vec![vec![None, None, Some("test:a".to_owned())]],
            "the two inline copies are two features, the named one is shared"
        );
        assert_eq!(built.per_biome[0][0].ones().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(built.per_biome[1][0].ones().collect::<Vec<_>>(), vec![0, 2]);
    }

    #[test]
    fn an_id_that_resolves_to_nothing_names_the_asset() {
        let corpus = corpus();
        let missing = steps(&["test:absent"]);
        let error = build_feature_steps(&[&missing], &corpus).unwrap_err();
        assert_eq!(
            error,
            FeatureCompileError::UnknownPlacedFeature(id("test:absent"))
        );
        assert_eq!(error.to_string(), "unknown placed feature: test:absent");
    }

    #[test]
    fn a_cycle_is_a_load_error_naming_a_feature_on_it() {
        let corpus = corpus();
        let first = steps(&["test:a", "test:b"]);
        let second = steps(&["test:b", "test:a"]);
        assert_eq!(
            build_feature_steps(&[&first, &second,], &corpus,).unwrap_err(),
            FeatureCompileError::FeatureCycle("test:a".to_owned())
        );
    }

    /// Resolves everything, so a corpus walk fails only where a shape has no
    /// compiled form at all.
    struct Everything;

    impl BlockResolver for Everything {
        fn state(&self, _state: &BlockState) -> Option<VoxelId> {
            Some(VoxelId(1))
        }

        fn states(&self, _query: StateQuery<'_>) -> Option<StateMask> {
            Some(Arc::new(FixedBitSet::with_capacity(1)))
        }

        fn biomes(&self, _set: &HolderSet) -> Option<BiomeMask> {
            Some(Arc::new(FixedBitSet::with_capacity(1)))
        }
    }

    #[test]
    fn every_shipped_placed_feature_compiles() {
        let placed: BTreeMap<ResourceLocation, PlacedFeature> =
            mcrs_minecraft_worldgen_testing::registry("placed_feature");
        assert_eq!(placed.len(), 274);
        for (id, feature) in &placed {
            compile_placement(&feature.placement, &Everything)
                .unwrap_or_else(|error| panic!("{id}: {error}"));
        }

        // `cuboid`, `randomly_selected` and `random_chance` appear only in the
        // placed features a feature inlines, never in the registry's own files.
        let features: BTreeMap<ResourceLocation, Feature> =
            mcrs_minecraft_worldgen_testing::registry("feature");
        let mut inline = 0;
        for (id, feature) in &features {
            let mut nested = Vec::new();
            feature.visit_placed_features(&mut |holder| nested.push(holder.clone()));
            for holder in nested {
                if let Holder::Inline(placed) = holder {
                    inline += 1;
                    compile_placement(&placed.placement, &Everything)
                        .unwrap_or_else(|error| panic!("{id}: {error}"));
                }
            }
        }
        assert!(inline > 0, "the corpus inlines placed features");
    }

    #[test]
    fn a_block_state_that_resolves_to_nothing_names_it() {
        struct Nothing;
        impl BlockResolver for Nothing {
            fn state(&self, _state: &BlockState) -> Option<VoxelId> {
                None
            }
            fn states(&self, _query: StateQuery<'_>) -> Option<StateMask> {
                None
            }
            fn biomes(&self, _set: &HolderSet) -> Option<BiomeMask> {
                None
            }
        }

        let predicate: BlockPredicate = serde_json::from_str(
            r#"{"type":"minecraft:would_survive","state":{"id":"test:absent","properties":{"stage":"1"}}}"#,
        )
        .unwrap();
        let error = compile_predicate(&predicate, &Nothing).unwrap_err();
        assert_eq!(
            error.to_string(),
            "unknown block state: test:absent[stage=1]"
        );

        let predicate: BlockPredicate = serde_json::from_str(
            r##"{"type":"minecraft:matching_block_tag","tag":"test:absent"}"##,
        )
        .unwrap();
        assert_eq!(
            compile_predicate(&predicate, &Nothing)
                .unwrap_err()
                .to_string(),
            "unknown block set: #test:absent"
        );
    }

    #[test]
    fn a_predicate_with_no_compiled_form_names_the_type() {
        let predicate: BlockPredicate =
            serde_json::from_str(r#"{"type":"minecraft:unobstructed"}"#).unwrap();
        assert_eq!(
            compile_predicate(&predicate, &Everything).unwrap_err(),
            FeatureCompileError::Unsupported("minecraft:unobstructed".to_owned())
        );
    }

    #[test]
    fn a_rule_test_resolves_its_blocks_and_tags() {
        let rule: RuleTest = serde_json::from_str(
            r#"{"predicate_type":"minecraft:random_block_match","block":"minecraft:blackstone","probability":0.25}"#,
        )
        .unwrap();
        assert!(matches!(
            compile_rule(&rule, &Everything).unwrap(),
            Rule::RandomStates { probability, .. } if probability == 0.25
        ));
    }
}
