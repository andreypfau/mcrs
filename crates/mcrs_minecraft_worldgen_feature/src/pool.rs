use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_registries::dispatch;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_worldgen_density::proto::Either;
use serde::{Deserialize, Serialize};

use crate::proto::{PlacedFeature, StructureProcessorList, WrappedProcessors};
use crate::template::Projection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidSettings {
    IgnoreWaterlogging,
    #[default]
    ApplyWaterlogging,
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

#[dispatch(registry = crate::keys::StructurePoolElementType, key = "element_type")]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum PoolElement {
    #[dispatch(SinglePoolElement)]
    Single(SingleElement),
    #[dispatch(LegacySinglePoolElement)]
    LegacySingle(SingleElement),
    #[dispatch(ListPoolElement)]
    List {
        elements: Vec<PoolElement>,
        projection: Projection,
    },
    #[dispatch(FeaturePoolElement)]
    Feature {
        feature: Holder<PlacedFeature>,
        projection: Projection,
    },
    #[dispatch(EmptyPoolElement)]
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

impl SingleElement {
    /// No processor list is an empty list written in place, not a reference.
    pub fn new(
        location: impl Into<ResourceLocation>,
        processors: Option<Holder<StructureProcessorList>>,
        projection: Projection,
    ) -> Self {
        SingleElement {
            location: location.into(),
            processors: processors.unwrap_or_else(|| {
                Holder::Inline(Box::new(StructureProcessorList(Either::Left(
                    WrappedProcessors {
                        processors: Vec::new(),
                    },
                ))))
            }),
            projection,
            override_liquid_settings: None,
        }
    }
}
