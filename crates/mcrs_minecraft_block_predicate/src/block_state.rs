use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::resource_location::InvalidResourceLocation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A block state as the noise settings write it: a bare block id, or an id with
/// stated property values.
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub struct BlockState {
    pub name: ResourceLocation,
    /// `None` where the file named the block alone, which is not the same as an
    /// empty property map: the two must serialize back to what they came from.
    pub properties: Option<BTreeMap<String, String>>,
}

impl From<Block> for BlockState {
    fn from(block: Block) -> Self {
        Self::bare(block.location().to_arc())
    }
}

impl BlockState {
    /// A block named alone, with no stated property values.
    pub fn bare(name: ResourceLocation) -> Self {
        BlockState {
            name,
            properties: None,
        }
    }

    pub fn minecraft(name: &str) -> Result<Self, InvalidResourceLocation> {
        Ok(Self::bare(ResourceLocation::minecraft(name)?))
    }

    pub fn with(mut self, property: &str, value: &str) -> Self {
        self.properties
            .get_or_insert_default()
            .insert(property.to_string(), value.to_string());
        self
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatedBlockState {
    id: ResourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}

impl<'de> Deserialize<'de> for BlockState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Form::deserialize(deserializer)? {
            Form::Bare(name) => Ok(BlockState::bare(name)),
            Form::Stated(state) => Ok(BlockState {
                name: state.id,
                properties: state.properties,
            }),
        }
    }
}

impl Serialize for BlockState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.properties {
            None => self.name.serialize(serializer),
            Some(_) => serialize_stated(self, serializer),
        }
    }
}

/// The object form even for a block named alone, where a bare id would read
/// back as something else.
pub(crate) fn serialize_stated<S: serde::Serializer>(
    state: &BlockState,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    StatedBlockState {
        id: state.name.clone(),
        properties: state.properties.clone(),
    }
    .serialize(serializer)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Form {
    Bare(ResourceLocation),
    Stated(StatedBlockState),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_block_id_round_trips_without_a_property_map() {
        let state: BlockState = serde_json::from_str(r#""minecraft:stone""#).unwrap();
        assert_eq!(state.name.as_str(), "minecraft:stone");
        assert!(state.properties.is_none());
        assert_eq!(
            serde_json::to_string(&state).unwrap(),
            r#""minecraft:stone""#
        );
    }

    #[test]
    fn a_stated_block_keeps_its_properties() {
        let state: BlockState =
            serde_json::from_str(r#"{"id":"minecraft:water","properties":{"level":"0"}}"#).unwrap();
        assert_eq!(state.properties.as_ref().unwrap()["level"], "0");
        assert_eq!(
            serde_json::to_string(&state).unwrap(),
            r#"{"id":"minecraft:water","properties":{"level":"0"}}"#
        );
    }

    #[test]
    fn a_provider_state_named_alone_keeps_its_object_form() {
        let json = r#"{"id":"minecraft:stone"}"#;
        let provider: crate::provider::BlockStateProvider = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&provider).unwrap(), json);
    }
}
