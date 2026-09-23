use std::collections::BTreeMap;

pub mod frozen;
pub mod hardcoded;
pub mod jigsaw;
pub mod locate;
pub mod orient;
pub mod piece;
pub mod placement;
pub mod site;

use serde::{Deserialize, Serialize};

use mcrs_minecraft_worldgen_feature::template::Projection;

use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{Bounded, NonNegativeInt, PositiveInt, is_default};
use mcrs_minecraft_core::value_provider::{HeightProvider, IntProvider, Weighted};
use mcrs_minecraft_worldgen_density::proto::Either;
use mcrs_minecraft_worldgen_feature::block_predicate::Offset;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::proto::{Holder, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_feature::tree::{PositiveFloat, UnitFloat, non_empty};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureSet {
    pub structures: Vec<StructureSelectionEntry>,
    pub placement: StructurePlacement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureSelectionEntry {
    pub structure: ResourceLocation,
    pub weight: PositiveInt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum StructurePlacement {
    #[serde(rename = "minecraft:random_spread")]
    RandomSpread {
        #[serde(flatten)]
        spreading: Spreading,
        spacing: Bounded<0, 4096>,
        separation: Bounded<0, 4096>,
        #[serde(default, skip_serializing_if = "is_default")]
        spread_type: SpreadType,
    },
    #[serde(rename = "minecraft:concentric_rings")]
    ConcentricRings {
        #[serde(flatten)]
        spreading: Spreading,
        distance: Bounded<0, 1023>,
        spread: Bounded<0, 1023>,
        count: Bounded<1, 4095>,
        preferred_biomes: HolderSet,
    },
    // An empty struct variant, not a unit one: only the former refuses extra keys.
    #[serde(rename = "minecraft:dimension_origin")]
    DimensionOrigin {},
}

// Flatten target: the enclosing enum reports unknown keys, so no `deny_unknown_fields` here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spreading {
    pub salt: NonNegativeInt,
    #[serde(default = "d_unit_1_0", skip_serializing_if = "is_unit_1_0")]
    pub frequency: UnitFloat,
    #[serde(default, skip_serializing_if = "is_default")]
    pub frequency_reduction_method: FrequencyReduction,
    #[serde(default, skip_serializing_if = "is_default")]
    pub locate_offset: Offset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusion_zone: Option<ExclusionZone>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExclusionZone {
    pub other_set: ResourceLocation,
    pub chunk_count: Bounded<1, 16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyReduction {
    #[default]
    Default,
    #[serde(rename = "legacy_type_1")]
    LegacyType1,
    #[serde(rename = "legacy_type_2")]
    LegacyType2,
    #[serde(rename = "legacy_type_3")]
    LegacyType3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpreadType {
    #[default]
    Linear,
    Triangular,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Structure {
    #[serde(rename = "minecraft:buried_treasure")]
    BuriedTreasure {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:desert_pyramid")]
    DesertPyramid {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:end_city")]
    EndCity {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:fortress")]
    Fortress {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:igloo")]
    Igloo {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:jigsaw")]
    Jigsaw {
        #[serde(flatten)]
        settings: StructureSettings,
        #[serde(flatten)]
        jigsaw: JigsawConfig,
    },
    #[serde(rename = "minecraft:jungle_temple")]
    JungleTemple {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:mineshaft")]
    Mineshaft {
        #[serde(flatten)]
        settings: StructureSettings,
        mineshaft_type: MineshaftType,
    },
    #[serde(rename = "minecraft:nether_fossil")]
    NetherFossil {
        #[serde(flatten)]
        settings: StructureSettings,
        height: HeightProvider,
    },
    #[serde(rename = "minecraft:ocean_monument")]
    OceanMonument {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:ocean_ruin")]
    OceanRuin {
        #[serde(flatten)]
        settings: StructureSettings,
        biome_temp: OceanTemperature,
        large_probability: UnitFloat,
        cluster_probability: UnitFloat,
    },
    #[serde(rename = "minecraft:ruined_portal")]
    RuinedPortal {
        #[serde(flatten)]
        settings: StructureSettings,
        #[serde(deserialize_with = "non_empty")]
        setups: Vec<RuinedPortalSetup>,
    },
    #[serde(rename = "minecraft:shipwreck")]
    Shipwreck {
        #[serde(flatten)]
        settings: StructureSettings,
        is_beached: bool,
    },
    #[serde(rename = "minecraft:stronghold")]
    Stronghold {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:swamp_hut")]
    SwampHut {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    #[serde(rename = "minecraft:woodland_mansion")]
    WoodlandMansion {
        #[serde(flatten)]
        settings: StructureSettings,
    },
}

impl mcrs_minecraft_core::tag_key::TaggedRegistry for Structure {
    const REGISTRY_PATH: &'static str = "worldgen/structure";
}

impl Structure {
    pub fn settings(&self) -> &StructureSettings {
        match self {
            Structure::BuriedTreasure { settings }
            | Structure::DesertPyramid { settings }
            | Structure::EndCity { settings }
            | Structure::Fortress { settings }
            | Structure::Igloo { settings }
            | Structure::Jigsaw { settings, .. }
            | Structure::JungleTemple { settings }
            | Structure::Mineshaft { settings, .. }
            | Structure::NetherFossil { settings, .. }
            | Structure::OceanMonument { settings }
            | Structure::OceanRuin { settings, .. }
            | Structure::RuinedPortal { settings, .. }
            | Structure::Shipwreck { settings, .. }
            | Structure::Stronghold { settings }
            | Structure::SwampHut { settings }
            | Structure::WoodlandMansion { settings } => settings,
        }
    }

    /// The `minecraft:` template paths the type's generator draws from, which
    /// no datapack file names.
    pub fn templates(&self) -> &'static [&'static str] {
        match self {
            Structure::EndCity { .. } => hardcoded::end_city::TEMPLATES,
            Structure::Igloo { .. } => hardcoded::igloo::TEMPLATES,
            Structure::NetherFossil { .. } => hardcoded::nether_fossil::TEMPLATES,
            Structure::OceanRuin { .. } => hardcoded::ocean_ruin::TEMPLATES,
            Structure::RuinedPortal { .. } => hardcoded::ruined_portal::TEMPLATES,
            Structure::Shipwreck { .. } => hardcoded::shipwreck::TEMPLATES,
            Structure::WoodlandMansion { .. } => hardcoded::woodland_mansion::TEMPLATES,
            _ => &[],
        }
    }
}

// Flatten target: no `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureSettings {
    pub biomes: HolderSet,
    pub spawn_overrides: BTreeMap<MobCategory, SpawnOverride>,
    pub step: DecorationStep,
    #[serde(default, skip_serializing_if = "is_default")]
    pub terrain_adaptation: TerrainAdaptation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobCategory {
    Monster,
    Creature,
    Ambient,
    Axolotls,
    UndergroundWaterCreature,
    WaterCreature,
    WaterAmbient,
    Misc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnOverride {
    pub bounding_box: SpawnBoundingBox,
    pub spawns: Vec<SpawnerData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnBoundingBox {
    Piece,
    Full,
}

// The weight sits beside the entry's own fields, not under `data` as in `Weighted<T>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnerData {
    #[serde(rename = "type")]
    pub entity: ResourceLocation,
    pub count: IntProvider,
    pub weight: NonNegativeInt,
}

// Declaration order is the step index; keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorationStep {
    RawGeneration,
    Lakes,
    LocalModifications,
    UndergroundStructures,
    SurfaceStructures,
    Strongholds,
    UndergroundOres,
    UndergroundDecoration,
    FluidSprings,
    VegetalDecoration,
    TopLayerModification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainAdaptation {
    #[default]
    None,
    Bury,
    BeardThin,
    BeardBox,
    Encapsulate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MineshaftType {
    Normal,
    Mesa,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OceanTemperature {
    Warm,
    Cold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuinedPortalSetup {
    pub placement: PortalPlacement,
    pub air_pocket_probability: UnitFloat,
    pub mossiness: UnitFloat,
    pub overgrown: bool,
    pub vines: bool,
    pub can_be_cold: bool,
    pub replace_with_blackstone: bool,
    pub weight: PositiveFloat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortalPlacement {
    OnLandSurface,
    PartlyBuried,
    OnOceanFloor,
    InMountain,
    Underground,
    InNether,
}

// Flatten target: no `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JigsawConfig {
    pub start_pool: ResourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_jigsaw_name: Option<ResourceLocation>,
    pub size: Bounded<0, 20>,
    pub start_height: HeightProvider,
    pub use_expansion_hack: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_start_to_heightmap: Option<HeightmapName>,
    pub max_distance_from_center: MaxDistance,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pool_aliases: Vec<PoolAlias>,
    #[serde(default = "d_padding_zero", skip_serializing_if = "is_padding_zero")]
    pub dimension_padding: DimensionPadding,
    #[serde(default, skip_serializing_if = "is_default")]
    pub liquid_settings: LiquidSettings,
}

/// A bare int sets both axes; the object form defaults `vertical` to the world height.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MaxDistance(pub Either<Bounded<1, 128>, MaxDistanceFull>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaxDistanceFull {
    pub horizontal: Bounded<1, 128>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub vertical: Bounded<1, 4064, 4064>,
}

impl MaxDistance {
    pub fn horizontal(&self) -> i32 {
        match &self.0 {
            Either::Left(both) => both.0,
            Either::Right(full) => full.horizontal.0,
        }
    }

    pub fn vertical(&self) -> i32 {
        match &self.0 {
            Either::Left(both) => both.0,
            Either::Right(full) => full.vertical.0,
        }
    }
}

/// A bare int pads both ends; the object form defaults each end to 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DimensionPadding(pub Either<NonNegativeInt, DimensionPaddingFull>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionPaddingFull {
    #[serde(default, skip_serializing_if = "is_default")]
    pub bottom: NonNegativeInt,
    #[serde(default, skip_serializing_if = "is_default")]
    pub top: NonNegativeInt,
}

impl DimensionPadding {
    pub fn bottom(&self) -> i32 {
        match &self.0 {
            Either::Left(both) => both.0,
            Either::Right(full) => full.bottom.0,
        }
    }

    pub fn top(&self) -> i32 {
        match &self.0 {
            Either::Left(both) => both.0,
            Either::Right(full) => full.top.0,
        }
    }
}

fn d_padding_zero() -> DimensionPadding {
    DimensionPadding(Either::Left(Bounded(0)))
}

fn is_padding_zero(padding: &DimensionPadding) -> bool {
    *padding == d_padding_zero()
}

fn d_unit_1_0() -> UnitFloat {
    UnitFloat(1.0)
}

fn is_unit_1_0(value: &UnitFloat) -> bool {
    *value == UnitFloat(1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidSettings {
    IgnoreWaterlogging,
    #[default]
    ApplyWaterlogging,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum PoolAlias {
    #[serde(rename = "minecraft:direct")]
    Direct {
        alias: ResourceLocation,
        target: ResourceLocation,
    },
    #[serde(rename = "minecraft:random")]
    Random {
        alias: ResourceLocation,
        #[serde(deserialize_with = "non_empty")]
        targets: Vec<Weighted<ResourceLocation>>,
    },
    #[serde(rename = "minecraft:random_group")]
    RandomGroup {
        #[serde(deserialize_with = "non_empty")]
        groups: Vec<Weighted<Vec<PoolAlias>>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplatePool {
    pub fallback: ResourceLocation,
    pub elements: Vec<PoolEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolEntry {
    pub element: PoolElement,
    pub weight: Bounded<1, 150>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "element_type", deny_unknown_fields)]
pub enum PoolElement {
    #[serde(rename = "minecraft:single_pool_element")]
    Single(SingleElement),
    #[serde(rename = "minecraft:legacy_single_pool_element")]
    LegacySingle(SingleElement),
    #[serde(rename = "minecraft:list_pool_element")]
    List {
        elements: Vec<PoolElement>,
        projection: Projection,
    },
    #[serde(rename = "minecraft:feature_pool_element")]
    Feature {
        feature: Holder<PlacedFeature>,
        projection: Projection,
    },
    #[serde(rename = "minecraft:empty_pool_element")]
    Empty {},
}

// The newtype variants hand this the whole map, so it must refuse unknown keys itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SingleElement {
    pub location: ResourceLocation,
    pub processors: Holder<StructureProcessorList>,
    pub projection: Projection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_liquid_settings: Option<LiquidSettings>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_testing::round_trips;

    #[test]
    fn every_shipped_set_structure_and_pool_round_trips() {
        assert_eq!(round_trips::<StructureSet>("structure_set"), 21);
        assert_eq!(round_trips::<Structure>("structure"), 52);
        assert_eq!(round_trips::<TemplatePool>("template_pool"), 245);
    }

    fn round_trip<T: serde::de::DeserializeOwned + Serialize>(json: &str) -> T {
        let parsed: T = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        parsed
    }

    #[test]
    fn dimension_origin_has_no_fields() {
        let set: StructureSet = round_trip(
            r#"{"structures":[{"structure":"minecraft:a","weight":1}],"placement":{"type":"minecraft:dimension_origin"}}"#,
        );
        assert_eq!(set.placement, StructurePlacement::DimensionOrigin {});
        assert!(
            serde_json::from_str::<StructurePlacement>(
                r#"{"type":"minecraft:dimension_origin","salt":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn jigsaw_object_forms_and_explicit_defaults() {
        let json = r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:has_structure/x","spawn_overrides":{},"step":"surface_structures","start_pool":"minecraft:x/start","size":7,"start_height":{"absolute":0},"use_expansion_hack":false,"max_distance_from_center":{"horizontal":80,"vertical":64},"dimension_padding":{"bottom":3,"top":5}}"##;
        let Structure::Jigsaw { jigsaw, .. } = round_trip::<Structure>(json) else {
            panic!("not a jigsaw");
        };
        assert_eq!(
            (
                jigsaw.max_distance_from_center.horizontal(),
                jigsaw.max_distance_from_center.vertical()
            ),
            (80, 64)
        );
        assert_eq!(
            (
                jigsaw.dimension_padding.bottom(),
                jigsaw.dimension_padding.top()
            ),
            (3, 5)
        );

        let explicit_defaults = r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:has_structure/x","spawn_overrides":{},"step":"surface_structures","terrain_adaptation":"none","start_pool":"minecraft:x/start","size":7,"start_height":{"absolute":0},"use_expansion_hack":true,"max_distance_from_center":{"horizontal":80,"vertical":4064},"dimension_padding":0,"liquid_settings":"apply_waterlogging","pool_aliases":[]}"##;
        let parsed: Structure = serde_json::from_str(explicit_defaults).unwrap();
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:has_structure/x","spawn_overrides":{},"step":"surface_structures","start_pool":"minecraft:x/start","size":7,"start_height":{"absolute":0},"use_expansion_hack":true,"max_distance_from_center":{"horizontal":80}}"##
        );
        let Structure::Jigsaw { jigsaw, .. } = parsed else {
            panic!("not a jigsaw");
        };
        assert_eq!(jigsaw.max_distance_from_center.vertical(), 4064);
        assert_eq!(
            (
                jigsaw.dimension_padding.bottom(),
                jigsaw.dimension_padding.top()
            ),
            (0, 0)
        );
    }

    #[test]
    fn spreading_explicit_defaults_are_dropped() {
        let parsed: StructurePlacement = serde_json::from_str(
            r#"{"type":"minecraft:random_spread","salt":1,"frequency":1.0,"frequency_reduction_method":"default","locate_offset":[0,0,0],"spacing":2,"separation":1,"spread_type":"linear"}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            r#"{"type":"minecraft:random_spread","salt":1,"spacing":2,"separation":1}"#
        );
    }

    #[test]
    fn override_liquid_settings_round_trips() {
        let entry: PoolEntry = round_trip(
            r#"{"element":{"element_type":"minecraft:single_pool_element","location":"minecraft:x/y","processors":"minecraft:empty","projection":"rigid","override_liquid_settings":"ignore_waterlogging"},"weight":1}"#,
        );
        let PoolElement::Single(single) = entry.element else {
            panic!("not single");
        };
        assert_eq!(
            single.override_liquid_settings,
            Some(LiquidSettings::IgnoreWaterlogging)
        );
        assert!(
            serde_json::from_str::<PoolElement>(
                r#"{"element_type":"minecraft:empty_pool_element","projection":"rigid"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn unknown_keys_under_a_flatten_are_refused() {
        for json in [
            r##"{"type":"minecraft:igloo","biomes":"#minecraft:x","spawn_overrides":{},"step":"lakes","bogus":1}"##,
            r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:x","spawn_overrides":{},"step":"lakes","start_pool":"minecraft:p","size":1,"start_height":{"absolute":0},"use_expansion_hack":true,"max_distance_from_center":80,"bogus":1}"##,
            r#"{"type":"minecraft:random_spread","salt":1,"spacing":2,"separation":1,"bogus":1}"#,
        ] {
            assert!(serde_json::from_str::<serde_json::Value>(json).is_ok());
            assert!(
                serde_json::from_str::<Structure>(json).is_err()
                    && serde_json::from_str::<StructurePlacement>(json).is_err(),
                "{json}"
            );
        }
    }

    #[test]
    fn decoration_step_ordinals_match_the_registry() {
        assert_eq!(DecorationStep::RawGeneration as usize, 0);
        assert_eq!(DecorationStep::SurfaceStructures as usize, 4);
        assert_eq!(DecorationStep::TopLayerModification as usize, 10);
    }
}
