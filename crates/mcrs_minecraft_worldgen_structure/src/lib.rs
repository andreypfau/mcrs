use std::collections::BTreeMap;

#[rustfmt::skip]
pub mod keys;
pub mod blueprint;
pub mod frozen;
pub mod hardcoded;
pub mod jigsaw;
pub mod locate;
pub mod orient;
pub mod piece;
pub mod placement;
pub mod site;
pub mod spawn_condition;

use serde::{Deserialize, Serialize};

use mcrs_minecraft_worldgen_feature::pool::LiquidSettings;

use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use mcrs_minecraft_block_predicate::predicate::Offset;
use mcrs_minecraft_block_predicate::provider::{PositiveFloat, UnitFloat, non_empty};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{Bounded, NonNegativeInt, PositiveInt, is_default};
use mcrs_minecraft_entity::spawn::{MobCategory, SpawnerData};
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_value_provider::{HeightProvider, Weighted};
use mcrs_minecraft_worldgen_density::proto::Either;
use mcrs_minecraft_worldgen_feature::placement::DecorationStep;

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
#[serde(remote = "Self", deny_unknown_fields)]
pub enum StructurePlacement {
    RandomSpread {
        #[serde(flatten)]
        spreading: Spreading,
        spacing: Bounded<0, 4096>,
        separation: Bounded<0, 4096>,
        #[serde(default, skip_serializing_if = "is_default")]
        spread_type: SpreadType,
    },
    ConcentricRings {
        #[serde(flatten)]
        spreading: Spreading,
        distance: Bounded<0, 1023>,
        spread: Bounded<0, 1023>,
        count: Bounded<1, 4095>,
        preferred_biomes: HolderSet<Biome>,
    },
    // An empty struct variant, not a unit one: only the former refuses extra keys.
    DimensionOrigin {},
}

mcrs_minecraft_registry::dispatch! {
    StructurePlacement, key = "type", registry = crate::keys::StructurePlacementType,
    {
        ConcentricRings => ConcentricRings,
        DimensionOrigin => DimensionOrigin,
        RandomSpread => RandomSpread,
    }
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
#[serde(remote = "Self", deny_unknown_fields)]
pub enum Structure {
    BuriedTreasure {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    DesertPyramid {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    EndCity {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    Fortress {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    Igloo {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    Jigsaw {
        #[serde(flatten)]
        settings: StructureSettings,
        #[serde(flatten)]
        jigsaw: JigsawConfig,
    },
    JungleTemple {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    Mineshaft {
        #[serde(flatten)]
        settings: StructureSettings,
        mineshaft_type: MineshaftType,
    },
    NetherFossil {
        #[serde(flatten)]
        settings: StructureSettings,
        height: HeightProvider,
    },
    OceanMonument {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    OceanRuin {
        #[serde(flatten)]
        settings: StructureSettings,
        biome_temp: OceanTemperature,
        large_probability: UnitFloat,
        cluster_probability: UnitFloat,
    },
    RuinedPortal {
        #[serde(flatten)]
        settings: StructureSettings,
        #[serde(deserialize_with = "non_empty")]
        setups: Vec<RuinedPortalSetup>,
    },
    Shipwreck {
        #[serde(flatten)]
        settings: StructureSettings,
        is_beached: bool,
    },
    Stronghold {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    SwampHut {
        #[serde(flatten)]
        settings: StructureSettings,
    },
    WoodlandMansion {
        #[serde(flatten)]
        settings: StructureSettings,
    },
}

mcrs_minecraft_registry::dispatch! {
    Structure, key = "type", registry = crate::keys::StructureType,
    {
        BuriedTreasure => BuriedTreasure,
        DesertPyramid => DesertPyramid,
        EndCity => EndCity,
        Fortress => Fortress,
        Igloo => Igloo,
        Jigsaw => Jigsaw,
        JungleTemple => JungleTemple,
        Mineshaft => Mineshaft,
        NetherFossil => NetherFossil,
        OceanMonument => OceanMonument,
        OceanRuin => OceanRuin,
        RuinedPortal => RuinedPortal,
        Shipwreck => Shipwreck,
        Stronghold => Stronghold,
        SwampHut => SwampHut,
        WoodlandMansion => WoodlandMansion,
    }
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
    pub biomes: HolderSet<Biome>,
    pub spawn_overrides: BTreeMap<MobCategory, SpawnOverride>,
    pub step: DecorationStep,
    #[serde(default, skip_serializing_if = "is_default")]
    pub terrain_adaptation: TerrainAdaptation,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum PoolAlias {
    Direct {
        alias: ResourceLocation,
        target: ResourceLocation,
    },
    Random {
        alias: ResourceLocation,
        #[serde(deserialize_with = "non_empty")]
        targets: Vec<Weighted<ResourceLocation>>,
    },
    RandomGroup {
        #[serde(deserialize_with = "non_empty")]
        groups: Vec<Weighted<Vec<PoolAlias>>>,
    },
}

mcrs_minecraft_registry::dispatch! {
    PoolAlias, key = "type", registry = crate::keys::PoolAliasBindingType,
    {
        Random => Random,
        RandomGroup => RandomGroup,
        Direct => Direct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_feature::pool::{PoolElement, PoolEntry, TemplatePool};
    use mcrs_minecraft_worldgen_testing::{corpus_set, round_trips};

    #[test]
    fn every_shipped_set_structure_and_pool_round_trips() {
        assert_eq!(round_trips::<StructureSet>("structure_set"), 21);
        assert_eq!(round_trips::<Structure>("structure"), 52);
        assert_eq!(round_trips::<TemplatePool>("template_pool"), 245);
    }

    fn round_trip<T: serde::de::DeserializeOwned + Serialize>(json: &str) -> T {
        corpus_set().scope(|| {
            let parsed: T = serde_json::from_str(json).unwrap();
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
            parsed
        })
    }

    type Codec = fn(&str) -> Result<String, String>;

    fn codec<T: serde::de::DeserializeOwned + Serialize>(json: &str) -> Result<String, String> {
        corpus_set().scope(|| {
            let value: T = serde_json::from_str(json).map_err(|error| error.to_string())?;
            Ok(serde_json::to_string(&value).unwrap())
        })
    }

    #[test]
    fn explicit_defaults_are_dropped_and_the_rest_is_kept() {
        let cases: &[(Codec, &str, &str)] = &[
            (
                codec::<StructurePlacement>,
                r#"{"type":"minecraft:random_spread","salt":1,"frequency":1.0,"frequency_reduction_method":"default","locate_offset":[0,0,0],"spacing":2,"separation":1,"spread_type":"linear"}"#,
                r#"{"type":"minecraft:random_spread","salt":1,"spacing":2,"separation":1}"#,
            ),
            (
                codec::<PoolEntry>,
                r#"{"element":{"element_type":"minecraft:single_pool_element","location":"minecraft:x/y","processors":"minecraft:empty","projection":"rigid","override_liquid_settings":"ignore_waterlogging"},"weight":1}"#,
                r#"{"element":{"element_type":"minecraft:single_pool_element","location":"minecraft:x/y","processors":"minecraft:empty","projection":"rigid","override_liquid_settings":"ignore_waterlogging"},"weight":1}"#,
            ),
        ];
        for (codec, json, expected) in cases {
            assert_eq!(codec(json).as_deref(), Ok(*expected));
        }
    }

    #[test]
    fn a_field_the_type_does_not_declare_is_refused() {
        let cases: &[(Codec, &str, &str)] = &[
            (
                codec::<Structure>,
                r##"{"type":"minecraft:igloo","biomes":"#minecraft:is_overworld","spawn_overrides":{},"step":"lakes"}"##,
                r#""bogus":1"#,
            ),
            (
                codec::<Structure>,
                r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:is_overworld","spawn_overrides":{},"step":"lakes","start_pool":"minecraft:p","size":1,"start_height":{"absolute":0},"use_expansion_hack":true,"max_distance_from_center":80}"##,
                r#""bogus":1"#,
            ),
            (
                codec::<StructurePlacement>,
                r#"{"type":"minecraft:random_spread","salt":1,"spacing":2,"separation":1}"#,
                r#""bogus":1"#,
            ),
            (
                codec::<PoolElement>,
                r#"{"element_type":"minecraft:empty_pool_element"}"#,
                r#""projection":"rigid""#,
            ),
        ];
        for (codec, json, extra) in cases {
            codec(json).unwrap();
            let with_extra = format!("{},{extra}}}", &json[..json.len() - 1]);
            assert!(codec(&with_extra).is_err(), "{with_extra}");
        }
    }

    #[test]
    fn jigsaw_object_forms_and_explicit_defaults() {
        let json = r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:has_structure/village_plains","spawn_overrides":{},"step":"surface_structures","start_pool":"minecraft:x/start","size":7,"start_height":{"absolute":0},"use_expansion_hack":false,"max_distance_from_center":{"horizontal":80,"vertical":64},"dimension_padding":{"bottom":3,"top":5}}"##;
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

        let explicit_defaults = r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:has_structure/village_plains","spawn_overrides":{},"step":"surface_structures","terrain_adaptation":"none","start_pool":"minecraft:x/start","size":7,"start_height":{"absolute":0},"use_expansion_hack":true,"max_distance_from_center":{"horizontal":80,"vertical":4064},"dimension_padding":0,"liquid_settings":"apply_waterlogging","pool_aliases":[]}"##;
        let parsed: Structure =
            corpus_set().scope(|| serde_json::from_str(explicit_defaults).unwrap());
        assert_eq!(
            corpus_set().scope(|| serde_json::to_string(&parsed).unwrap()),
            r##"{"type":"minecraft:jigsaw","biomes":"#minecraft:has_structure/village_plains","spawn_overrides":{},"step":"surface_structures","start_pool":"minecraft:x/start","size":7,"start_height":{"absolute":0},"use_expansion_hack":true,"max_distance_from_center":{"horizontal":80}}"##
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
}
