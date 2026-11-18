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

pub(crate) struct NamedRegistry {
    pub(crate) table: NameTable,
    pub(crate) key: &'static str,
}

pub(crate) const NAMED_REGISTRIES: [NamedRegistry; 14] = [
    NamedRegistry {
        table: NameTable::Attribute,
        key: "minecraft:attribute",
    },
    NamedRegistry {
        table: NameTable::Block,
        key: "minecraft:block",
    },
    NamedRegistry {
        table: NameTable::ComponentType,
        key: "minecraft:data_component_type",
    },
    NamedRegistry {
        table: NameTable::Enchantment,
        key: "minecraft:enchantment",
    },
    NamedRegistry {
        table: NameTable::MapDecorationType,
        key: "minecraft:map_decoration_type",
    },
    NamedRegistry {
        table: NameTable::SoundEvent,
        key: "minecraft:sound_event",
    },
    NamedRegistry {
        table: NameTable::EntityType,
        key: "minecraft:entity_type",
    },
    NamedRegistry {
        table: NameTable::MobEffect,
        key: "minecraft:mob_effect",
    },
    NamedRegistry {
        table: NameTable::Potion,
        key: "minecraft:potion",
    },
    NamedRegistry {
        table: NameTable::TrimMaterial,
        key: "minecraft:trim_material",
    },
    NamedRegistry {
        table: NameTable::TrimPattern,
        key: "minecraft:trim_pattern",
    },
    NamedRegistry {
        table: NameTable::Instrument,
        key: "minecraft:instrument",
    },
    NamedRegistry {
        table: NameTable::BannerPattern,
        key: "minecraft:banner_pattern",
    },
    NamedRegistry {
        table: NameTable::PaintingVariant,
        key: "minecraft:painting_variant",
    },
];

fn registry_key(table: NameTable) -> Option<&'static str> {
    NAMED_REGISTRIES
        .iter()
        .find(|named| named.table == table)
        .map(|named| named.key)
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
