use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;
use std::collections::HashMap;

/// The names of the loaded registries, for the codecs that read and write them as identifiers.
/// The item names are not here: the table that bakes the items gives them.
pub(crate) struct RegistryNames<'registries> {
    registries: &'registries Registries,
    block_tags: Vec<ResourceLocation<'registries>>,
    block_tag_ids: HashMap<ResourceLocation<'registries>, u16>,
}

impl<'registries> RegistryNames<'registries> {
    pub(crate) fn new(registries: &'registries Registries) -> Self {
        let block_tags: Vec<_> = registries
            .tags_of(BLOCK)
            .into_iter()
            .flatten()
            .map(|(tag, _)| ResourceLocation::from(tag))
            .collect();
        let block_tag_ids = block_tags
            .iter()
            .enumerate()
            .filter_map(|(position, tag)| Some((*tag, u16::try_from(position).ok()?)))
            .collect();
        Self {
            registries,
            block_tags,
            block_tag_ids,
        }
    }
}

const ENCHANTMENT: ResourceLocation<'static> = rl!("minecraft:enchantment");
const ATTRIBUTE: ResourceLocation<'static> = rl!("minecraft:attribute");
const BLOCK: ResourceLocation<'static> = rl!("minecraft:block");
const COMPONENT_TYPE: ResourceLocation<'static> = rl!("minecraft:data_component_type");

fn registry(table: NameTable) -> Option<ResourceLocation<'static>> {
    match table {
        NameTable::Enchantment => Some(ENCHANTMENT),
        NameTable::Attribute => Some(ATTRIBUTE),
        NameTable::Block | NameTable::BlockTag => Some(BLOCK),
        NameTable::ComponentType => Some(COMPONENT_TYPE),
        _ => None,
    }
}

impl NameLookup for RegistryNames<'_> {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        let registry = registry(table)?;
        if table == NameTable::BlockTag {
            return self.block_tag_ids.get(&name).copied();
        }
        self.registries.id_of(registry, name)
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        let registry = registry(table)?;
        if table == NameTable::BlockTag {
            return self.block_tags.get(usize::from(id)).copied();
        }
        self.registries
            .names_of(registry)?
            .get(usize::from(id))
            .map(ResourceLocation::from)
    }
}
