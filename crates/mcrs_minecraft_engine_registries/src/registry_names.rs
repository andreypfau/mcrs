use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::TagKey;

pub struct RegistryNames<'a> {
    registries: &'a Registries,
    items: Option<&'a ItemTable>,
    block_tags: Option<Tags<Block>>,
    entity_type_tags: Option<Tags<EntityType>>,
}

impl<'a> RegistryNames<'a> {
    #[must_use]
    pub fn new(registries: &'a Registries, items: &'a ItemTable) -> Self {
        Self {
            registries,
            items: Some(items),
            block_tags: registries.tags::<Block>(),
            entity_type_tags: registries.tags::<EntityType>(),
        }
    }

    pub(crate) fn without_items(registries: &'a Registries) -> Self {
        Self {
            registries,
            items: None,
            block_tags: registries.tags::<Block>(),
            entity_type_tags: registries.tags::<EntityType>(),
        }
    }
}

fn registry_key(table: NameTable) -> Option<&'static str> {
    match table {
        NameTable::Attribute => Some("minecraft:attribute"),
        NameTable::Block => Some("minecraft:block"),
        NameTable::ComponentType => Some("minecraft:data_component_type"),
        NameTable::Enchantment => Some("minecraft:enchantment"),
        NameTable::MapDecorationType => Some("minecraft:map_decoration_type"),
        NameTable::SoundEvent => Some("minecraft:sound_event"),
        NameTable::EntityType => Some("minecraft:entity_type"),
        _ => None,
    }
}

fn tag_id<R>(tags: &Tags<R>, name: ResourceLocation<'_>) -> Option<u16> {
    let tag = tags.id(TagKey::new(name))?;
    u16::try_from(tag.index()).ok()
}

fn tag_name<R>(tags: &Tags<R>, id: u16) -> Option<ResourceLocation<'_>> {
    let (_, name) = tags
        .iter()
        .find(|(tag, _)| tag.index() == usize::from(id))?;
    Some(name.as_location())
}

impl NameLookup for RegistryNames<'_> {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        if table == NameTable::Item {
            return self.items?.id(table, name);
        }
        if table == NameTable::BlockTag {
            return tag_id(self.block_tags.as_ref()?, name);
        }
        if table == NameTable::EntityTypeTag {
            return tag_id(self.entity_type_tags.as_ref()?, name);
        }
        let key = ResourceLocation::read(registry_key(table)?).ok()?;
        self.registries.id_of(key, name)
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        if table == NameTable::Item {
            return self.items?.name(table, id);
        }
        if table == NameTable::BlockTag {
            return tag_name(self.block_tags.as_ref()?, id);
        }
        if table == NameTable::EntityTypeTag {
            return tag_name(self.entity_type_tags.as_ref()?, id);
        }
        let key = ResourceLocation::read(registry_key(table)?).ok()?;
        let names = self.registries.names_of(key)?;
        Some(names.get(usize::from(id))?.as_location())
    }
}
