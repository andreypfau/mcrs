use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_registries::{KeySet, Named, dispatch};
use std::fmt;

use serde::de::Error as _;
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::placement::IntOr;
use super::placement::{PlacementModifier, VerticalDirection};
use super::rule_test::RuleTest;
use super::tree::TreeConfig;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_block_predicate::fluid_state::FluidState;
use mcrs_minecraft_block_predicate::predicate::BlockPredicate;
use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use mcrs_minecraft_block_predicate::provider::{BlockStateProvider, UnitFloat};
use mcrs_minecraft_core::Axis;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_core::codec::{Bounded, NonNegativeInt, default_true, is_default, non_empty};
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_value_provider::{BoundedIntProvider, FloatProvider, IntProvider, Weighted};
use mcrs_minecraft_worldgen_density::proto::Either;
use mcrs_minecraft_worldgen_surface::proto::CaveSurface;

/// `RegistryCodecs.holderSet(PLACED_FEATURE, …)`: a `#tag`, one entry or a
/// list of them, each a registry id or an inline placed feature. A tag keeps
/// its name and is expanded when the features compile; a one-entry list
/// writes as the bare entry.
#[derive(Debug, Clone, PartialEq)]
pub enum PlacedFeatureSet {
    Tag(ResourceLocation),
    List(Vec<Holder<PlacedFeature>>),
}

impl PlacedFeatureSet {
    /// The entries written in the file; a tag lists none.
    pub fn listed(&self) -> &[Holder<PlacedFeature>] {
        match self {
            PlacedFeatureSet::Tag(_) => &[],
            PlacedFeatureSet::List(entries) => entries,
        }
    }

    fn tagged<E: serde::de::Error>(text: &str) -> Result<Self, E> {
        let tag = text.strip_prefix('#').unwrap_or(text);
        ResourceLocation::read(tag)
            .map(PlacedFeatureSet::Tag)
            .map_err(E::custom)
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
                    .map(|only| PlacedFeatureSet::List(vec![only]))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Holder::deserialize(value::MapAccessDeserializer::new(map))
                    .map(|only| PlacedFeatureSet::List(vec![only]))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(PlacedFeatureSet::List)
            }
        }

        deserializer.deserialize_any(SetVisitor)
    }
}

impl Serialize for PlacedFeatureSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            PlacedFeatureSet::Tag(tag) => serializer.collect_str(&format_args!("#{tag}")),
            PlacedFeatureSet::List(entries) => match entries.as_slice() {
                [only] => only.serialize(serializer),
                entries => entries.serialize(serializer),
            },
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
            | Self::SimpleRandomSelector { features } => features.listed().iter().for_each(visit),
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
            | Self::Crevasse { .. }
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
            | Self::LargeSpeleothem { .. }
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

impl mcrs_minecraft_resource::RegistryValue for PlacedFeature {
    type Registry = mcrs_minecraft_biome::PlacedFeature;
}

impl PlacedFeature {
    /// The horizontal range around its chunk's corner that the feature can
    /// place into.
    pub fn xz_domain(&self) -> std::ops::RangeInclusive<i32> {
        self.placement
            .iter()
            .fold(0..=0, |domain, modifier| modifier.xz_domain(domain))
    }
}

/// The 58 entries of `FeatureTypes`, and Beta's populate step. A 26.3 feature
/// writes its configuration into the same object as its `type`; there is no
/// `config` wrapper.
#[dispatch(registry = crate::keys::FeatureType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum Feature {
    Bamboo {
        probability: UnitFloat,
    },
    /// Beta's populate step. One legacy stream per chunk runs every object in
    /// it, so the step is one feature rather than one per object.
    #[dispatch(id = "mcrs:beta_populate")]
    BetaPopulate,
    BlockBlob {
        state: BlockState,
        can_place_on: BlockPredicate<Named>,
    },
    BlockColumn {
        layers: Vec<BlockColumnLayer>,
        direction: Direction,
        allowed_placement: BlockPredicate<Named>,
        prioritize_tip: bool,
    },
    BlockPile {
        state_provider: BlockStateProvider<Named>,
    },
    BlueIce,
    BonusChest,
    ChorusPlant,
    CoralClaw {
        feature: Holder<PlacedFeature>,
    },
    Crevasse {
        max_width: Bounded<2, 8>,
        max_length: Bounded<4, 16>,
        min_footprint: Bounded<1, 32>,
        cursor_surface_search_range: Bounded<1, { i32::MAX }>,
        column_surface_search_range: Bounded<1, { i32::MAX }>,
        twistiness: Twistiness,
        tapering: UnitFloat,
        turn_factor: UnitFloat,
        carve_depth: BoundedIntProvider<1, { i32::MAX }>,
        fill_depth: BoundedIntProvider<1, { i32::MAX }>,
        surface_block: BlockStateProvider<Named>,
        fill_block: BlockStateProvider<Named>,
        carvable_blocks: BlockPredicate<Named>,
        acceptable_barrier: BlockPredicate<Named>,
    },
    CoralTree {
        feature: Holder<PlacedFeature>,
    },
    #[dispatch(DeltaFeature)]
    Delta {
        contents: BlockState,
        rim: BlockState,
        size: BoundedIntProvider<0, 16>,
        rim_size: BoundedIntProvider<0, 16>,
    },
    Disk {
        state_provider: BlockStateProvider<Named>,
        target: BlockPredicate<Named>,
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
    #[dispatch(EndSpike)]
    EndSpikes {
        spikes: Vec<EndSpike>,
        #[serde(default, skip_serializing_if = "is_default")]
        crystal_invulnerable: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crystal_beam_target: Option<[i32; 3]>,
    },
    FallenTree {
        trunk_provider: BlockStateProvider<Named>,
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
        cap_provider: BlockStateProvider<Named>,
        stem_provider: BlockStateProvider<Named>,
        #[serde(default, skip_serializing_if = "is_default")]
        foliage_radius: IntOr<2>,
        can_place_on: BlockPredicate<Named>,
    },
    HugeFungus {
        valid_base_block: BlockState,
        stem_state: BlockState,
        hat_state: BlockState,
        decor_state: BlockState,
        replaceable_blocks: BlockPredicate<Named>,
        #[serde(default, skip_serializing_if = "is_default")]
        planted: bool,
    },
    HugeRedMushroom {
        cap_provider: BlockStateProvider<Named>,
        stem_provider: BlockStateProvider<Named>,
        #[serde(default, skip_serializing_if = "is_default")]
        foliage_radius: IntOr<2>,
        can_place_on: BlockPredicate<Named>,
    },
    Iceberg {
        state: BlockState,
    },
    Lake {
        fluid: BlockStateProvider<Named>,
        barrier: BlockStateProvider<Named>,
        can_place_feature: BlockPredicate<Named>,
        can_replace_with_air_or_fluid: BlockPredicate<Named>,
        can_replace_with_barrier: BlockPredicate<Named>,
    },
    LargeSpeleothem {
        base_block: BlockState,
        replaceable_blocks: KeySet<Block>,
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
        can_be_placed_on: KeySet<Block>,
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
        block: BlockStateProvider<Named>,
        project_through: BlockPredicate<Named>,
        size: BoundedIntProvider<1, 16>,
        max_projection_height: NonNegativeInt,
    },
    RandomBooleanSelector {
        feature_true: Holder<PlacedFeature>,
        feature_false: Holder<PlacedFeature>,
    },
    RandomNeighborSpread {
        block: BlockStateProvider<Named>,
        accepted_neighbors: KeySet<Block>,
        can_replace: BlockPredicate<Named>,
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
        root_replaceable: KeySet<Block>,
        root_state_provider: BlockStateProvider<Named>,
        root_placement_attempts: Bounded<1, 256>,
        root_column_max_height: Bounded<1, 4096>,
        hanging_root_radius: Bounded<1, 64>,
        hanging_roots_vertical_span: Bounded<1, 16>,
        hanging_root_state_provider: BlockStateProvider<Named>,
        hanging_root_placement_attempts: Bounded<1, 256>,
        allowed_vertical_water_for_tree: Bounded<1, 64>,
        allowed_tree_position: BlockPredicate<Named>,
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
        to_place: BlockStateProvider<Named>,
        #[serde(default, skip_serializing_if = "is_default")]
        schedule_tick: bool,
    },
    SimpleRandomSelector {
        #[serde(deserialize_with = "non_empty_set")]
        features: PlacedFeatureSet,
    },
    SingleBlockPillar {
        block: BlockStateProvider<Named>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        can_replace: Option<BlockPredicate<Named>>,
        direction: VerticalDirection,
        #[serde(default = "d_unit_1_0", skip_serializing_if = "is_unit_1_0")]
        chance_to_continue: UnitFloat,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cap_feature: Option<Holder<PlacedFeature>>,
    },
    Speleothem {
        base_block: BlockState,
        pointed_block: BlockState,
        replaceable_blocks: KeySet<Block>,
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
        replaceable_blocks: KeySet<Block>,
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
        #[serde(default, skip_serializing_if = "is_default")]
        placement_options: SpeleothemPlacementOptions,
    },
    Spike {
        state: BlockState,
        can_place_on: BlockPredicate<Named>,
        can_replace: BlockPredicate<Named>,
    },
    #[dispatch(SpringFeature)]
    Spring {
        state: FluidState,
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        requires_block_below: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        rock_count: IntOr<4>,
        #[serde(default, skip_serializing_if = "is_default")]
        hole_count: IntOr<1>,
        valid_blocks: KeySet<Block>,
    },
    SteppedColumnCluster {
        block: BlockStateProvider<Named>,
        continue_through: BlockPredicate<Named>,
        can_replace: BlockPredicate<Named>,
        cannot_place_on: KeySet<Block>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeleothemPlacementOptions {
    pub placement_mode: SpeleothemPlacementMode,
    pub base_block_transformer: SpeleothemBaseTransformer,
    pub allow_water_placement: bool,
}

impl Default for SpeleothemPlacementOptions {
    fn default() -> Self {
        SpeleothemPlacementOptions {
            placement_mode: SpeleothemPlacementMode::FloorAndCeiling,
            base_block_transformer: SpeleothemBaseTransformer::None,
            allow_water_placement: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeleothemPlacementMode {
    FloorAndCeiling,
    FloorOnly,
    CeilingOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeleothemBaseTransformer {
    None,
    SetAttached,
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
    pub provider: BlockStateProvider<Named>,
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
    pub replaceable: KeySet<Block>,
    pub ground_state: BlockStateProvider<Named>,
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
    pub filling_provider: BlockStateProvider<Named>,
    pub inner_layer_provider: BlockStateProvider<Named>,
    pub alternate_inner_layer_provider: BlockStateProvider<Named>,
    pub middle_layer_provider: BlockStateProvider<Named>,
    pub outer_layer_provider: BlockStateProvider<Named>,
    #[serde(deserialize_with = "non_empty")]
    pub inner_placements: Vec<BlockState>,
    pub cannot_replace: KeySet<Block>,
    pub invalid_blocks: KeySet<Block>,
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

#[dispatch(registry = crate::keys::StructureProcessorType, key = "processor_type")]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
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
        rottable_blocks: Option<KeySet<Block>>,
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
        value: KeySet<Block>,
    },
    Rule {
        rules: Vec<ProcessorRule>,
    },
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

#[dispatch(registry = crate::keys::PosRuleTestType, key = "predicate_type")]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[dispatch(registry = crate::keys::RuleBlockEntityModifierType, unsupported(AppendStatic))]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum RuleBlockEntityModifier {
    Clear,
    Passthrough,
    AppendLoot { loot_table: ResourceLocation },
}

// ---------------------------------------------------------------------------
// Bounded scalars, defaults and validators
// ---------------------------------------------------------------------------

mcrs_minecraft_noise::bounded_float! {
    UnitDouble as f64 in [0.0, 1.0];
    CrackSize as f64 in [0.0, 5.0];
    LayerThickness as f64 in [0.01, 50.0];
    CaveHeightRatio as f64 in [0.1, 1.0];
    BluntnessForWind as f64 in [0.0, 5.0];
    Twistiness as f32 in [0.0, 16.0];
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
    if matches!(&set, PlacedFeatureSet::List(entries) if entries.is_empty()) {
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
        assert_eq!(round_trips::<Feature>("feature"), 262);
        assert_eq!(round_trips::<PlacedFeature>("placed_feature"), 300);
        assert_eq!(round_trips::<StructureProcessorList>("processor_list"), 40);
    }
}
