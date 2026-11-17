use mcrs_minecraft_enchantment::file::EnchantmentFile;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{EnchantmentId, Kind};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocation;

#[derive(Debug, Clone)]
pub(crate) struct ItemRules {
    pub(crate) items: ItemTable,
    pub(crate) enchantments: Box<[EnchantmentRule]>,
    pub(crate) book: Option<Kind>,
    pub(crate) enchanted_book: Option<Kind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnchantmentRule {
    pub(crate) id: EnchantmentId,
    pub(crate) max_level: u32,
    pub(crate) supported: Box<[Kind]>,
}

impl EnchantmentRule {
    pub(crate) fn supports(&self, kind: Kind) -> bool {
        self.supported.binary_search(&kind).is_ok()
    }
}

impl ItemRules {
    pub(crate) fn bake(registries: &Registries, items: &ItemTable) -> Self {
        let item_tags = registries.tags::<Item>();
        let enchantments = registries
            .registry::<EnchantmentFile>()
            .into_iter()
            .flat_map(|files| files.iter())
            .filter_map(|(id, _, file)| {
                let mut supported: Vec<Kind> = match &item_tags {
                    Some(tags) => file
                        .supported_items
                        .ids(tags)
                        .map(|item| Kind(item.number()))
                        .collect(),
                    None => Vec::new(),
                };
                supported.sort_unstable();
                supported.dedup();
                Some(EnchantmentRule {
                    id: EnchantmentId(u16::try_from(id.index()).ok()?),
                    max_level: file.max_level,
                    supported: supported.into_boxed_slice(),
                })
            })
            .collect();
        let kind = |name: &str| {
            let name = ResourceLocation::read(name).ok()?;
            let item = registries.entries::<Item>()?.by_name(name)?;
            Some(Kind(item.number()))
        };
        Self {
            items: *items,
            enchantments,
            book: kind("minecraft:book"),
            enchanted_book: kind("minecraft:enchanted_book"),
        }
    }

    pub(crate) fn enchantment(&self, id: EnchantmentId) -> Option<&EnchantmentRule> {
        self.enchantments.get(usize::from(id.0))
    }
}
