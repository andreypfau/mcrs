use mcrs_minecraft_block_predicate::provider::Holder;
use std::fmt;

use serde::de::Error as _;
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::placement::IntOr;
use super::placement::{PlacementModifier, VerticalDirection};
use super::rule_test::RuleTest;
use super::tree::TreeConfig;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_block_predicate::predicate::BlockPredicate;
use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use mcrs_minecraft_block_predicate::provider::{BlockSet, BlockStateProvider, UnitFloat};
use mcrs_minecraft_core::Axis;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_core::codec::{Bounded, NonNegativeInt, default_true, is_default, non_empty};
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_registry::{HolderSet, Registry, TagId, Tags};
use mcrs_minecraft_value_provider::{BoundedIntProvider, FloatProvider, IntProvider, Weighted};
use mcrs_minecraft_worldgen_density::proto::Either;
use mcrs_minecraft_worldgen_surface::proto::CaveSurface;

/// `RegistryCodecs.holderSet(PLACED_FEATURE, …)`: a `#tag`, one entry or a
/// list of them, each a registry id or an inline placed feature. A tag is
/// expanded from the set the file is read in and writes back as the tag; a
/// one-entry list writes as the bare entry.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacedFeatureSet {
    tag: Option<TagId<PlacedFeature>>,
    entries: Vec<Holder<PlacedFeature>>,
}

impl PlacedFeatureSet {
    pub fn entries(&self) -> &[Holder<PlacedFeature>] {
        &self.entries
    }

    fn listed(entries: Vec<Holder<PlacedFeature>>) -> Self {
        PlacedFeatureSet { tag: None, entries }
    }

    fn tagged<E: serde::de::Error>(text: &str) -> Result<Self, E> {
        let tag = match HolderSet::<PlacedFeature>::deserialize(value::StrDeserializer::new(text))?
        {
            HolderSet::Named(tag) => tag,
            // `skip_sets` reads every set as empty.
            _ => return Ok(PlacedFeatureSet::default()),
        };
        let entries = Tags::<PlacedFeature>::in_scope("a placed feature tag", |tags| {
            Registry::<PlacedFeature>::in_scope("a placed feature tag", |names| {
                tags.members(tag)
                    .map(|id| {
                        Holder::Reference(names.name(id).expect("a tag member has a name").clone())
                    })
                    .collect()
            })
        })
        .and_then(|entries| entries)
        .map_err(E::custom)?;
        Ok(PlacedFeatureSet {
            tag: Some(tag),
            entries,
        })
    }
}

impl<'de> Deserialize<'de> for PlacedFeatureSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor;

        impl<'de> Visitor<'de> for SetVisitor {
            type Value = PlacedFeatureSet;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a placed feature tag, a placed feature, or a list of them")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                if text.starts_with('#') {
                    return PlacedFeatureSet::tagged(text);
                }
                Holder::deserialize(value::StrDeserializer::new(text))
                    .map(|only| PlacedFeatureSet::listed(vec![only]))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Holder::deserialize(value::MapAccessDeserializer::new(map))
                    .map(|only| PlacedFeatureSet::listed(vec![only]))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq))
                    .map(PlacedFeatureSet::listed)
            }
        }

        deserializer.deserialize_any(SetVisitor)
    }
}

impl Serialize for PlacedFeatureSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(tag) = self.tag {
            return HolderSet::Named(tag).serialize(serializer);
        }
        match self.entries.as_slice() {
            [only] => only.serialize(serializer),
            entries => entries.serialize(serializer),
        }
    }
}

/// One decoration step of a biome: its placed features in order, every tag
/// already expanded.
pub type FeatureStepList = Vec<Holder<PlacedFeature>>;

impl Feature {
    /// The structure templates this feature places.
    pub fn templates(&self) -> impl Iterator<Item = &ResourceLocation> {
        let (entries, fossils, overlays): (&[Weighted<TemplateEntry>], &[_], &[_]) = match self {
            Self::Template { templates, .. } => (templates, &[], &[]),
            Self::Fossil {
                fossil_structures,
                overlay_structures,
                ..
            } => (&[], fossil_structures, overlay_structures),
            _ => (&[], &[], &[]),
        };
        entries
            .iter()
            .map(|entry| &entry.data.id)
            .chain(fossils)
            .chain(overlays)
    }

    /// The placed features this one names, whether by id or written inline.
    /// Exhaustive on purpose: a feature added without a branch here would load
    /// with its selector targets missing and no error anywhere.
    pub fn visit_placed_features(&self, visit: &mut impl FnMut(&Holder<PlacedFeature>)) {
        match self {
            Self::CoralClaw { feature } | Self::CoralTree { feature } => visit(feature),
            Self::Overlay { features }
            | Self::Sequence { features }
            | Self::SimpleRandomSelector { features } => features.entries().iter().for_each(visit),
            Self::RandomBooleanSelector {
                feature_true,
                feature_false,
            } => {
                visit(feature_true);
                visit(feature_false);
            }
            Self::RandomSelector { features, default } => {
                for entry in features {
                    visit(&entry.feature);
                }
                visit(default);
            }
            Self::RootSystem { feature, .. } => visit(feature),
            Self::SingleBlockPillar { cap_feature, .. } => {
                if let Some(feature) = cap_feature {
                    visit(feature);
                }
            }
            Self::VegetationPatch(config) | Self::WaterloggedVegetationPatch(config) => {
                visit(&config.vegetation_feature)
            }
            Self::WeightedRandomSelector { features } => {
                for entry in features {
                    visit(&entry.data);
                }
            }
            Self::Bamboo { .. }
            | Self::BetaPopulate
            | Self::BlockBlob { .. }
            | Self::BlockColumn { .. }
            | Self::BlockPile { .. }
            | Self::BlueIce { .. }
            | Self::BonusChest { .. }
            | Self::ChorusPlant { .. }
            | Self::Delta { .. }
            | Self::Disk { .. }
            | Self::EndGateway { .. }
            | Self::EndIsland { .. }
            | Self::EndPlatform { .. }
            | Self::EndPodium { .. }
            | Self::EndSpikes { .. }
            | Self::FallenTree { .. }
            | Self::FillLayer { .. }
            | Self::Fossil { .. }
            | Self::FreezeTopLayer { .. }
            | Self::Geode { .. }
            | Self::HugeBrownMushroom { .. }
            | Self::HugeFungus { .. }
            | Self::HugeRedMushroom { .. }
            | Self::Iceberg { .. }
            | Self::Lake { .. }
            | Self::LargeDripstone { .. }
            | Self::MonsterRoom { .. }
            | Self::MultifaceGrowth { .. }
            | Self::NetherrackReplaceBlobs { .. }
            | Self::NoOp { .. }
            | Self::Ore { .. }
            | Self::ProjectedRandomPatchySquare { .. }
            | Self::RandomNeighborSpread { .. }
            | Self::ReplaceSingleBlock { .. }
            | Self::ScatteredOre { .. }
            | Self::SculkPatch { .. }
            | Self::SimpleBlock { .. }
            | Self::Speleothem { .. }
            | Self::SpeleothemCluster { .. }
            | Self::Spike { .. }
            | Self::Spring { .. }
            | Self::SteppedColumnCluster { .. }
            | Self::Template { .. }
            | Self::Tree { .. }
            | Self::UnderwaterMagma { .. }
            | Self::Vines { .. }
            | Self::VoidStartPlatform { .. } => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacedFeature {
    pub feature: Holder<Feature>,
    pub placement: Vec<PlacementModifier>,
}

impl RegistryValue for PlacedFeature {
    type Registry = Self;
}

/// The 58 entries of `FeatureTypes`, and Beta's populate step. A 26.3 feature
/// writes its configuration into the same object as its `type`; there is no
/// `config` wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum Feature {
    Bamboo {
        probability: UnitFloat,
    },
    /// Beta's populate step. One legacy stream per chunk runs every object in
    /// it, so the step is one feature rather than one per object.
    BetaPopulate,
    BlockBlob {
        state: BlockState,
        can_place_on: BlockPredicate,
    },
    BlockColumn {
        layers: Vec<BlockColumnLayer>,
        direction: Direction,
        allowed_placement: BlockPredicate,
        prioritize_tip: bool,
    },
    BlockPile {
        state_provider: BlockStateProvider,
    },
    BlueIce,
    BonusChest,
    ChorusPlant,
    CoralClaw {
        feature: Holder<PlacedFeature>,
    },
    CoralTree {
        feature: Holder<PlacedFeature>,
    },
    Delta {
        contents: BlockState,
        rim: BlockState,
        size: BoundedIntProvider<0, 16>,
        rim_size: BoundedIntProvider<0, 16>,
    },
    Disk {
        state_provider: BlockStateProvider,
        target: BlockPredicate,
        radius: BoundedIntProvider<0, 8>,
        half_height: Bounded<0, 4>,
    },
    EndGateway {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit: Option<[i32; 3]>,
        exact: bool,
    },
    EndIsland,
    EndPlatform,
    EndPodium {
        #[serde(default, skip_serializing_if = "is_default")]
        active: bool,
    },
    EndSpikes {
        spikes: Vec<EndSpike>,
        #[serde(default, skip_serializing_if = "is_default")]
        crystal_invulnerable: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crystal_beam_target: Option<[i32; 3]>,
    },
    FallenTree {
        trunk_provider: BlockStateProvider,
        log_length: IntProvider,
        stump_decorators: Vec<super::tree::TreeDecorator>,
        log_decorators: Vec<super::tree::TreeDecorator>,
    },
    FillLayer {
        height: Bounded<0, 4064>,
        state: BlockState,
    },
    Fossil {
        #[serde(deserialize_with = "non_empty")]
        fossil_structures: Vec<ResourceLocation>,
        #[serde(deserialize_with = "non_empty")]
        overlay_structures: Vec<ResourceLocation>,
        fossil_processors: Holder<StructureProcessorList>,
        overlay_processors: Holder<StructureProcessorList>,
        max_empty_corners_allowed: Bounded<0, 7>,
    },
    FreezeTopLayer,
    Geode {
        blocks: GeodeBlockSettings,
        layers: GeodeLayerSettings,
        crack: GeodeCrackSettings,
        #[serde(default = "d_chance_0_35", skip_serializing_if = "is_chance_0_35")]
        use_potential_placements_chance: UnitDouble,
        #[serde(default = "d_chance_0_0", skip_serializing_if = "is_chance_0_0")]
        use_alternate_layer0_chance: UnitDouble,
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        placements_require_layer0_alternate: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outer_wall_distance: Option<IntProvider>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        distribution_points: Option<IntProvider>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        point_offset: Option<IntProvider>,
        #[serde(default, skip_serializing_if = "is_default")]
        min_gen_offset: IntOr<-16>,
        #[serde(default, skip_serializing_if = "is_default")]
        max_gen_offset: IntOr<16>,
        #[serde(default = "d_chance_0_05", skip_serializing_if = "is_chance_0_05")]
        noise_multiplier: UnitDouble,
        invalid_blocks_threshold: i32,
    },
    HugeBrownMushroom {
        cap_provider: BlockStateProvider,
        stem_provider: BlockStateProvider,
        #[serde(default, skip_serializing_if = "is_default")]
        foliage_radius: IntOr<2>,
        can_place_on: BlockPredicate,
    },
    HugeFungus {
        valid_base_block: BlockState,
        stem_state: BlockState,
        hat_state: BlockState,
        decor_state: BlockState,
        replaceable_blocks: BlockPredicate,
        #[serde(default, skip_serializing_if = "is_default")]
        planted: bool,
    },
    HugeRedMushroom {
        cap_provider: BlockStateProvider,
        stem_provider: BlockStateProvider,
        #[serde(default, skip_serializing_if = "is_default")]
        foliage_radius: IntOr<2>,
        can_place_on: BlockPredicate,
    },
    Iceberg {
        state: BlockState,
    },
    Lake {
        fluid: BlockStateProvider,
        barrier: BlockStateProvider,
        can_place_feature: BlockPredicate,
        can_replace_with_air_or_fluid: BlockPredicate,
        can_replace_with_barrier: BlockPredicate,
    },
    LargeDripstone {
        replaceable_blocks: BlockSet,
        #[serde(default, skip_serializing_if = "is_default")]
        floor_to_ceiling_search_range: Bounded<1, 512, 30>,
        column_radius: BoundedIntProvider<1, 16>,
        height_scale: FloatProvider,
        max_column_radius_to_cave_height_ratio: CaveHeightRatio,
        stalactite_bluntness: FloatProvider,
        stalagmite_bluntness: FloatProvider,
        wind_speed: FloatProvider,
        min_radius_for_wind: Bounded<0, 100>,
        min_bluntness_for_wind: BluntnessForWind,
    },
    MonsterRoom,
    MultifaceGrowth {
        block: ResourceLocation,
        #[serde(default, skip_serializing_if = "is_default")]
        search_range: Bounded<1, 64, 10>,
        #[serde(default, skip_serializing_if = "is_default")]
        can_place_on_floor: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        can_place_on_ceiling: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        can_place_on_wall: bool,
        #[serde(default = "d_unit_0_5", skip_serializing_if = "is_unit_0_5")]
        chance_of_spreading: UnitFloat,
        can_be_placed_on: BlockSet,
    },
    NetherrackReplaceBlobs {
        target: BlockState,
        state: BlockState,
        radius: BoundedIntProvider<0, 12>,
    },
    NoOp,
    Ore {
        targets: Vec<BlockReplacement>,
        size: Bounded<0, 64>,
        discard_chance_on_air_exposure: UnitFloat,
    },
    Overlay {
        #[serde(deserialize_with = "non_empty_set")]
        features: PlacedFeatureSet,
    },
    ProjectedRandomPatchySquare {
        block: BlockStateProvider,
        project_through: BlockPredicate,
        size: BoundedIntProvider<1, 16>,
        max_projection_height: NonNegativeInt,
    },
    RandomBooleanSelector {
        feature_true: Holder<PlacedFeature>,
        feature_false: Holder<PlacedFeature>,
    },
    RandomNeighborSpread {
        block: BlockStateProvider,
        accepted_neighbors: BlockSet,
        can_replace: BlockPredicate,
        attempts: BoundedIntProvider<1, 3000>,
        xz_offset: BoundedIntProvider<-16, 16>,
        y_offset: BoundedIntProvider<-16, 16>,
    },
    RandomSelector {
        features: Vec<WeightedPlacedFeature>,
        default: Holder<PlacedFeature>,
    },
    ReplaceSingleBlock {
        targets: Vec<BlockReplacement>,
    },
    RootSystem {
        feature: Holder<PlacedFeature>,
        required_vertical_space_for_tree: Bounded<1, 64>,
        level_test_distance: Bounded<0, 16>,
        max_level_deviation: Bounded<0, 64>,
        root_radius: Bounded<1, 64>,
        root_replaceable: BlockSet,
        root_state_provider: BlockStateProvider,
        root_placement_attempts: Bounded<1, 256>,
        root_column_max_height: Bounded<1, 4096>,
        hanging_root_radius: Bounded<1, 64>,
        hanging_roots_vertical_span: Bounded<1, 16>,
        hanging_root_state_provider: BlockStateProvider,
        hanging_root_placement_attempts: Bounded<1, 256>,
        allowed_vertical_water_for_tree: Bounded<1, 64>,
        allowed_tree_position: BlockPredicate,
    },
    ScatteredOre {
        targets: Vec<BlockReplacement>,
        size: Bounded<0, 64>,
        discard_chance_on_air_exposure: UnitFloat,
    },
    SculkPatch {
        charge_count: Bounded<1, 32>,
        amount_per_charge: Bounded<1, 500>,
        spread_attempts: Bounded<1, 64>,
        growth_rounds: Bounded<0, 8>,
        spread_rounds: Bounded<0, 8>,
    },
    Sequence {
        #[serde(deserialize_with = "non_empty_set")]
        features: PlacedFeatureSet,
    },
    SimpleBlock {
        to_place: BlockStateProvider,
        #[serde(default, skip_serializing_if = "is_default")]
        schedule_tick: bool,
    },
    SimpleRandomSelector {
        #[serde(deserialize_with = "non_empty_set")]
        features: PlacedFeatureSet,
    },
    SingleBlockPillar {
        block: BlockStateProvider,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        can_replace: Option<BlockPredicate>,
        direction: VerticalDirection,
        #[serde(default = "d_unit_1_0", skip_serializing_if = "is_unit_1_0")]
        chance_to_continue: UnitFloat,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cap_feature: Option<Holder<PlacedFeature>>,
    },
    Speleothem {
        base_block: BlockState,
        pointed_block: BlockState,
        replaceable_blocks: BlockSet,
        #[serde(default = "d_unit_0_2", skip_serializing_if = "is_unit_0_2")]
        chance_of_taller_generation: UnitFloat,
        #[serde(default = "d_unit_0_7", skip_serializing_if = "is_unit_0_7")]
        chance_of_directional_spread: UnitFloat,
        #[serde(default = "d_unit_0_5", skip_serializing_if = "is_unit_0_5")]
        chance_of_spread_radius2: UnitFloat,
        #[serde(default = "d_unit_0_5", skip_serializing_if = "is_unit_0_5")]
        chance_of_spread_radius3: UnitFloat,
    },
    SpeleothemCluster {
        base_block: BlockState,
        pointed_block: BlockState,
        replaceable_blocks: BlockSet,
        floor_to_ceiling_search_range: Bounded<1, 512>,
        height: BoundedIntProvider<1, 128>,
        radius: BoundedIntProvider<1, 128>,
        max_stalagmite_stalactite_height_diff: Bounded<0, 64>,
        height_deviation: Bounded<1, 64>,
        speleothem_block_layer_thickness: BoundedIntProvider<0, 128>,
        density: FloatProvider,
        wetness: FloatProvider,
        chance_of_speleothem_at_max_distance_from_center: UnitFloat,
        max_distance_from_edge_affecting_chance_of_speleothem: Bounded<1, 64>,
        max_distance_from_center_affecting_height_bias: Bounded<1, 64>,
    },
    Spike {
        state: BlockState,
        can_place_on: BlockPredicate,
        can_replace: BlockPredicate,
    },
    Spring {
        state: BlockState,
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        requires_block_below: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        rock_count: IntOr<4>,
        #[serde(default, skip_serializing_if = "is_default")]
        hole_count: IntOr<1>,
        valid_blocks: BlockSet,
    },
    SteppedColumnCluster {
        block: BlockStateProvider,
        continue_through: BlockPredicate,
        can_replace: BlockPredicate,
        cannot_place_on: BlockSet,
        cluster_reach: IntProvider,
        column_count: IntProvider,
        column_reach: IntProvider,
        height: IntProvider,
    },
    Template {
        templates: Vec<Weighted<TemplateEntry>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        processors: Option<Holder<StructureProcessorList>>,
    },
    Tree(TreeConfig),
    UnderwaterMagma {
        floor_search_range: Bounded<0, 512>,
        placement_radius_around_floor: Bounded<0, 64>,
        placement_probability_per_valid_position: UnitFloat,
    },
    VegetationPatch(VegetationPatchConfig),
    Vines,
    VoidStartPlatform,
    WaterloggedVegetationPatch(VegetationPatchConfig),
    WeightedRandomSelector {
        features: Vec<Weighted<Holder<PlacedFeature>>>,
    },
}

mcrs_minecraft_registry::dispatch! {
    Feature, key = "type", registry = crate::keys::FeatureType,
    {
        Bamboo => Bamboo,
        BlockBlob => BlockBlob,
        BlockColumn => BlockColumn,
        BlockPile => BlockPile,
        BlueIce => BlueIce,
        BonusChest => BonusChest,
        ChorusPlant => ChorusPlant,
        CoralClaw => CoralClaw,
        CoralTree => CoralTree,
        DeltaFeature => Delta,
        Disk => Disk,
        EndGateway => EndGateway,
        EndIsland => EndIsland,
        EndPlatform => EndPlatform,
        EndPodium => EndPodium,
        EndSpike => EndSpikes,
        FallenTree => FallenTree,
        FillLayer => FillLayer,
        Fossil => Fossil,
        FreezeTopLayer => FreezeTopLayer,
        Geode => Geode,
        HugeBrownMushroom => HugeBrownMushroom,
        HugeFungus => HugeFungus,
        HugeRedMushroom => HugeRedMushroom,
        Iceberg => Iceberg,
        Lake => Lake,
        LargeDripstone => LargeDripstone,
        MonsterRoom => MonsterRoom,
        MultifaceGrowth => MultifaceGrowth,
        NetherrackReplaceBlobs => NetherrackReplaceBlobs,
        NoOp => NoOp,
        Ore => Ore,
        Overlay => Overlay,
        ProjectedRandomPatchySquare => ProjectedRandomPatchySquare,
        RandomBooleanSelector => RandomBooleanSelector,
        RandomNeighborSpread => RandomNeighborSpread,
        RandomSelector => RandomSelector,
        ReplaceSingleBlock => ReplaceSingleBlock,
        RootSystem => RootSystem,
        ScatteredOre => ScatteredOre,
        SculkPatch => SculkPatch,
        Sequence => Sequence,
        SimpleBlock => SimpleBlock,
        SimpleRandomSelector => SimpleRandomSelector,
        SingleBlockPillar => SingleBlockPillar,
        Speleothem => Speleothem,
        SpeleothemCluster => SpeleothemCluster,
        Spike => Spike,
        SpringFeature => Spring,
        SteppedColumnCluster => SteppedColumnCluster,
        Template => Template,
        Tree => Tree,
        UnderwaterMagma => UnderwaterMagma,
        VegetationPatch => VegetationPatch,
        Vines => Vines,
        VoidStartPlatform => VoidStartPlatform,
        WaterloggedVegetationPatch => WaterloggedVegetationPatch,
        WeightedRandomSelector => WeightedRandomSelector,
    }
    extend { "mcrs:beta_populate" => BetaPopulate }
}

impl RegistryValue for Feature {
    type Registry = Self;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockReplacement {
    pub target: RuleTest,
    pub state: BlockState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedPlacedFeature {
    pub feature: Holder<PlacedFeature>,
    pub chance: UnitFloat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockColumnLayer {
    pub height: IntProvider,
    pub provider: BlockStateProvider,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndSpike {
    #[serde(rename = "centerX", default, skip_serializing_if = "is_default")]
    pub center_x: i32,
    #[serde(rename = "centerZ", default, skip_serializing_if = "is_default")]
    pub center_z: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub radius: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub height: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub guarded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VegetationPatchConfig {
    pub replaceable: BlockSet,
    pub ground_state: BlockStateProvider,
    pub vegetation_feature: Holder<PlacedFeature>,
    pub surface: CaveSurface,
    pub depth: BoundedIntProvider<1, 128>,
    pub extra_bottom_block_chance: UnitFloat,
    pub vertical_range: Bounded<1, 256>,
    pub vegetation_chance: UnitFloat,
    pub xz_radius: IntProvider,
    pub extra_edge_column_chance: UnitFloat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateEntry {
    pub id: ResourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotations: Option<Vec<Rotation>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeodeBlockSettings {
    pub filling_provider: BlockStateProvider,
    pub inner_layer_provider: BlockStateProvider,
    pub alternate_inner_layer_provider: BlockStateProvider,
    pub middle_layer_provider: BlockStateProvider,
    pub outer_layer_provider: BlockStateProvider,
    #[serde(deserialize_with = "non_empty")]
    pub inner_placements: Vec<BlockState>,
    pub cannot_replace: BlockSet,
    pub invalid_blocks: BlockSet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeodeLayerSettings {
    #[serde(default = "d_layer_1_7", skip_serializing_if = "is_layer_1_7")]
    pub filling: LayerThickness,
    #[serde(default = "d_layer_2_2", skip_serializing_if = "is_layer_2_2")]
    pub inner_layer: LayerThickness,
    #[serde(default = "d_layer_3_2", skip_serializing_if = "is_layer_3_2")]
    pub middle_layer: LayerThickness,
    #[serde(default = "d_layer_4_2", skip_serializing_if = "is_layer_4_2")]
    pub outer_layer: LayerThickness,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeodeCrackSettings {
    #[serde(default = "d_chance_1_0", skip_serializing_if = "is_chance_1_0")]
    pub generate_crack_chance: UnitDouble,
    #[serde(default = "d_crack_2_0", skip_serializing_if = "is_crack_2_0")]
    pub base_crack_size: CrackSize,
    #[serde(default, skip_serializing_if = "is_default")]
    pub crack_point_offset: IntOr<2>,
}

// ---------------------------------------------------------------------------
// Structure processors, reached only through `fossil` and `template`
// ---------------------------------------------------------------------------

/// `StructureProcessorType.DIRECT_CODEC`: the list under a `processors` field,
/// or the bare list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StructureProcessorList(pub Either<WrappedProcessors, Vec<StructureProcessor>>);

impl RegistryValue for StructureProcessorList {
    type Registry = Self;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WrappedProcessors {
    pub processors: Vec<StructureProcessor>,
}

pub fn processor_list(list: &StructureProcessorList) -> &[StructureProcessor] {
    match &list.0 {
        Either::Left(wrapped) => &wrapped.processors,
        Either::Right(bare) => bare,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum StructureProcessor {
    BlackstoneReplace,
    BlockAge {
        mossiness: f64,
    },
    BlockIgnore {
        blocks: Vec<BlockState>,
    },
    BlockRot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rottable_blocks: Option<BlockSet>,
        integrity: UnitFloat,
    },
    Capped {
        delegate: Box<StructureProcessor>,
        limit: IntProvider,
    },
    Gravity {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        heightmap: Option<HeightmapName>,
        #[serde(default, skip_serializing_if = "is_default")]
        offset: i32,
    },
    JigsawReplacement,
    LavaSubmergedBlock,
    Nop,
    ProtectedBlocks {
        value: BlockSet,
    },
    Rule {
        rules: Vec<ProcessorRule>,
    },
}

mcrs_minecraft_registry::dispatch! {
    StructureProcessor, key = "processor_type", registry = crate::keys::StructureProcessorType,
    {
        BlackstoneReplace => BlackstoneReplace,
        BlockAge => BlockAge,
        BlockIgnore => BlockIgnore,
        BlockRot => BlockRot,
        Capped => Capped,
        Gravity => Gravity,
        JigsawReplacement => JigsawReplacement,
        LavaSubmergedBlock => LavaSubmergedBlock,
        Nop => Nop,
        ProtectedBlocks => ProtectedBlocks,
        Rule => Rule,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessorRule {
    pub input_predicate: RuleTest,
    pub location_predicate: RuleTest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_predicate: Option<PosRuleTest>,
    pub output_state: BlockState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_entity_modifier: Option<RuleBlockEntityModifier>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum PosRuleTest {
    AlwaysTrue,
    LinearPos(LinearPos),
    AxisAlignedLinearPos {
        #[serde(flatten)]
        linear: LinearPos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        axis: Option<Axis>,
    },
}

mcrs_minecraft_registry::dispatch! {
    PosRuleTest, key = "predicate_type", registry = crate::keys::PosRuleTestType,
    {
        AlwaysTrue => AlwaysTrue,
        LinearPos => LinearPos,
        AxisAlignedLinearPos => AxisAlignedLinearPos,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearPos {
    #[serde(default, skip_serializing_if = "is_default")]
    pub min_chance: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub max_chance: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub min_dist: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub max_dist: i32,
}

/// `append_static` is deliberately absent: its `data` is a `CompoundTag`, and
/// this crate has no NBT value type to hold one. A pack that uses it fails to
/// load naming the type, which is the wanted behaviour until templates exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum RuleBlockEntityModifier {
    Clear,
    Passthrough,
    AppendLoot { loot_table: ResourceLocation },
}

mcrs_minecraft_registry::dispatch! {
    RuleBlockEntityModifier, key = "type", registry = crate::keys::RuleBlockEntityModifierType,
    {
        Clear => Clear,
        Passthrough => Passthrough,
        AppendLoot => AppendLoot,
    }
    unsupported { AppendStatic }
}

// ---------------------------------------------------------------------------
// Bounded scalars, defaults and validators
// ---------------------------------------------------------------------------

mcrs_minecraft_worldgen_noise::bounded_float! {
    UnitDouble as f64 in [0.0, 1.0];
    CrackSize as f64 in [0.0, 5.0];
    LayerThickness as f64 in [0.01, 50.0];
    CaveHeightRatio as f64 in [0.1, 1.0];
    BluntnessForWind as f64 in [0.0, 5.0];
}

macro_rules! defaults {
    ($($get:ident / $is:ident -> $ty:ty = $value:expr;)*) => {$(
        fn $get() -> $ty {
            $value
        }

        fn $is(value: &$ty) -> bool {
            *value == $value
        }
    )*};
}

defaults! {
    d_unit_0_2 / is_unit_0_2 -> UnitFloat = UnitFloat(0.2);
    d_unit_0_5 / is_unit_0_5 -> UnitFloat = UnitFloat(0.5);
    d_unit_0_7 / is_unit_0_7 -> UnitFloat = UnitFloat(0.7);
    d_unit_1_0 / is_unit_1_0 -> UnitFloat = UnitFloat(1.0);
    d_chance_0_0 / is_chance_0_0 -> UnitDouble = UnitDouble(0.0);
    d_chance_0_05 / is_chance_0_05 -> UnitDouble = UnitDouble(0.05);
    d_chance_0_35 / is_chance_0_35 -> UnitDouble = UnitDouble(0.35);
    d_chance_1_0 / is_chance_1_0 -> UnitDouble = UnitDouble(1.0);
    d_crack_2_0 / is_crack_2_0 -> CrackSize = CrackSize(2.0);
    d_layer_1_7 / is_layer_1_7 -> LayerThickness = LayerThickness(1.7);
    d_layer_2_2 / is_layer_2_2 -> LayerThickness = LayerThickness(2.2);
    d_layer_3_2 / is_layer_3_2 -> LayerThickness = LayerThickness(3.2);
    d_layer_4_2 / is_layer_4_2 -> LayerThickness = LayerThickness(4.2);
}

fn non_empty_set<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PlacedFeatureSet, D::Error> {
    let set = PlacedFeatureSet::deserialize(deserializer)?;
    if set.tag.is_none() && set.entries.is_empty() {
        return Err(D::Error::custom("List must have contents"));
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_testing::round_trips;

    #[test]
    fn every_shipped_feature_and_placed_feature_round_trips() {
        assert_eq!(round_trips::<Feature>("feature"), 241);
        assert_eq!(round_trips::<PlacedFeature>("placed_feature"), 274);
        assert_eq!(round_trips::<StructureProcessorList>("processor_list"), 40);
    }
}
