use std::collections::BTreeMap;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;

use super::{
    DecorationStep, JigsawConfig, LiquidSettings, MineshaftType, OceanTemperature,
    RuinedPortalSetup, StructurePlacement, TerrainAdaptation,
};
use mcrs_minecraft_core::value_provider::HeightProvider;
use mcrs_minecraft_worldgen_feature::placer::BiomeMask;
use mcrs_minecraft_worldgen_feature::proto::{Holder, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_feature::template::Projection;
use mcrs_minecraft_worldgen_feature::template::{FrozenTemplate, TemplateManifest};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SetId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructureId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PoolId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateId(pub u32);

pub struct FrozenSet {
    pub id: ResourceLocation,
    pub placement: StructurePlacement,
    pub exclusion: Option<(SetId, i32)>,
    pub preferred_biomes: Option<BiomeMask>,
    pub entries: Vec<(StructureId, i32)>,
}

pub struct FrozenStructure {
    pub id: ResourceLocation,
    pub step: DecorationStep,
    pub step_index: u32,
    pub adaptation: TerrainAdaptation,
    pub biomes: BiomeMask,
    pub kind: StructureKind,
}

/// One variant per `StructureType`, each with its own `type`-specific fields
/// resolved against the registries.
#[derive(Debug, Clone, PartialEq)]
pub enum StructureKind {
    Jigsaw {
        start_pool: PoolId,
        config: JigsawConfig,
    },
    BuriedTreasure,
    DesertPyramid,
    EndCity,
    Fortress,
    Igloo,
    JungleTemple,
    Mineshaft {
        mineshaft_type: MineshaftType,
        /// `#minecraft:mineshaft_blocking`.
        blocking: BiomeMask,
    },
    NetherFossil {
        height: HeightProvider,
    },
    OceanMonument {
        /// `#minecraft:required_ocean_monument_surrounding`.
        surrounding: BiomeMask,
    },
    OceanRuin(OceanRuinConfig),
    RuinedPortal {
        setups: Vec<RuinedPortalSetup>,
        portals: Vec<TemplateId>,
        giant_portals: Vec<TemplateId>,
    },
    Shipwreck {
        is_beached: bool,
    },
    Stronghold,
    SwampHut,
    WoodlandMansion,
}

impl StructureKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            StructureKind::Jigsaw { .. } => "minecraft:jigsaw",
            StructureKind::BuriedTreasure => "minecraft:buried_treasure",
            StructureKind::DesertPyramid => "minecraft:desert_pyramid",
            StructureKind::EndCity => "minecraft:end_city",
            StructureKind::Fortress => "minecraft:fortress",
            StructureKind::Igloo => "minecraft:igloo",
            StructureKind::JungleTemple => "minecraft:jungle_temple",
            StructureKind::Mineshaft { .. } => "minecraft:mineshaft",
            StructureKind::NetherFossil { .. } => "minecraft:nether_fossil",
            StructureKind::OceanMonument { .. } => "minecraft:ocean_monument",
            StructureKind::OceanRuin(_) => "minecraft:ocean_ruin",
            StructureKind::RuinedPortal { .. } => "minecraft:ruined_portal",
            StructureKind::Shipwreck { .. } => "minecraft:shipwreck",
            StructureKind::Stronghold => "minecraft:stronghold",
            StructureKind::SwampHut => "minecraft:swamp_hut",
            StructureKind::WoodlandMansion => "minecraft:woodland_mansion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OceanRuinConfig {
    pub biome_temp: OceanTemperature,
    pub large_probability: f32,
    pub cluster_probability: f32,
}

pub struct FrozenPool {
    pub id: ResourceLocation,
    pub fallback: PoolId,
    /// Every element repeated `weight` times, in file order: what the shuffle draws from.
    pub expanded: Vec<ElementId>,
    pub max_size: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrozenElement {
    Single {
        template: TemplateId,
        legacy: bool,
        processors: Holder<StructureProcessorList>,
        projection: Projection,
        liquid_settings: Option<LiquidSettings>,
    },
    List {
        elements: Vec<ElementId>,
        projection: Projection,
    },
    Feature {
        feature: Holder<PlacedFeature>,
        projection: Projection,
    },
    Empty,
}

impl FrozenElement {
    pub fn projection(&self) -> Option<Projection> {
        match self {
            FrozenElement::Single { projection, .. }
            | FrozenElement::List { projection, .. }
            | FrozenElement::Feature { projection, .. } => Some(*projection),
            FrozenElement::Empty => None,
        }
    }
}

#[derive(Default)]
pub struct FrozenStructures {
    pub sets: Vec<FrozenSet>,
    /// Sorted by (path, namespace): the order `step_index` counts in.
    pub structures: Vec<FrozenStructure>,
    pub pools: Vec<FrozenPool>,
    pub elements: Vec<FrozenElement>,
    pub templates: Vec<Arc<FrozenTemplate>>,
    pub manifests: Vec<Arc<TemplateManifest>>,
    pub set_ids: BTreeMap<ResourceLocation, SetId>,
    pub structure_ids: BTreeMap<ResourceLocation, StructureId>,
    pub pool_ids: BTreeMap<ResourceLocation, PoolId>,
    pub template_ids: BTreeMap<ResourceLocation, TemplateId>,
}

pub struct DimensionStructureTables {
    pub frozen: Arc<FrozenStructures>,
    pub live: Vec<(SetId, Vec<StructureId>)>,
}

impl DimensionStructureTables {
    pub fn adapted(&self) -> impl Iterator<Item = &FrozenStructure> {
        self.live
            .iter()
            .flat_map(|(_, structures)| structures.iter())
            .map(|id| &self.frozen.structures[id.0 as usize])
            .filter(|structure| structure.adaptation != TerrainAdaptation::None)
    }
}
