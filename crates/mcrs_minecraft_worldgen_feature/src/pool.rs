use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
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

const STRUCTURE_POOL_ELEMENT_ROWS: &[&str] = &[
    "minecraft:single_pool_element",
    "minecraft:legacy_single_pool_element",
    "minecraft:list_pool_element",
    "minecraft:feature_pool_element",
    "minecraft:empty_pool_element",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    STRUCTURE_POOL_ELEMENT_ROWS,
    &[],
    crate::keys::StructurePoolElement::ENTRIES
));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_pool_element_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<PoolElement>(
            STRUCTURE_POOL_ELEMENT_ROWS,
            &[],
            crate::keys::StructurePoolElement::ENTRIES,
            |name| serde_json::json!({ "element_type": name }),
        );
    }
}
