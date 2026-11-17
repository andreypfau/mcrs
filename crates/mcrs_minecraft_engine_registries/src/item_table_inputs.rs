use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::registry_names::NAMED_REGISTRIES;

const ITEM_REGISTRY: &str = "minecraft:item";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemTableInputs {
    items: Vec<ResourceLocationBuf>,
    names: [Vec<ResourceLocationBuf>; NAMED_REGISTRIES.len()],
    block_tags: Vec<TagName>,
    entity_type_tags: Vec<TagName>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TagName {
    index: usize,
    name: ResourceLocationBuf,
}

impl ItemTableInputs {
    #[must_use]
    pub fn of(registries: &Registries) -> Self {
        Self {
            items: names(registries, ITEM_REGISTRY),
            names: NAMED_REGISTRIES.map(|named| names(registries, named.key)),
            block_tags: tag_names(registries.tags::<Block>()),
            entity_type_tags: tag_names(registries.tags::<EntityType>()),
        }
    }
}

fn names(registries: &Registries, key: &str) -> Vec<ResourceLocationBuf> {
    ResourceLocation::read(key)
        .ok()
        .and_then(|key| registries.names_of(key))
        .map(<[ResourceLocationBuf]>::to_vec)
        .unwrap_or_default()
}

fn tag_names<R>(tags: Option<Tags<R>>) -> Vec<TagName> {
    tags.map(|tags| {
        tags.iter()
            .map(|(tag, name)| TagName {
                index: tag.index(),
                name: name.clone(),
            })
            .collect()
    })
    .unwrap_or_default()
}
