use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::TagKey;

/// The names and ids that the item table reads, from the loaded registries. It has no item
/// names: the item table takes them from the item definitions.
pub(crate) struct RegistryNames<'a> {
    registries: &'a Registries,
    block_tags: Option<Tags<Block>>,
}

impl<'a> RegistryNames<'a> {
    pub(crate) fn new(registries: &'a Registries) -> Self {
        Self {
            registries,
            block_tags: registries.tags::<Block>(),
        }
    }
}

fn registry_key(table: NameTable) -> Option<&'static str> {
    match table {
        NameTable::Attribute => Some("minecraft:attribute"),
        NameTable::Block => Some("minecraft:block"),
        NameTable::ComponentType => Some("minecraft:data_component_type"),
        NameTable::Enchantment => Some("minecraft:enchantment"),
        _ => None,
    }
}

impl NameLookup for RegistryNames<'_> {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        if table == NameTable::BlockTag {
            let tag = self.block_tags.as_ref()?.id(TagKey::new(name))?;
            return u16::try_from(tag.index()).ok();
        }
        let key = ResourceLocation::read(registry_key(table)?).ok()?;
        self.registries.id_of(key, name)
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        if table == NameTable::BlockTag {
            let tags = self.block_tags.as_ref()?;
            let (_, name) = tags
                .iter()
                .find(|(tag, _)| tag.index() == usize::from(id))?;
            return Some(name.as_location());
        }
        let key = ResourceLocation::read(registry_key(table)?).ok()?;
        let names = self.registries.names_of(key)?;
        Some(names.get(usize::from(id))?.as_location())
    }
}
