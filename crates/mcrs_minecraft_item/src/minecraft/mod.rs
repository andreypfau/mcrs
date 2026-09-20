use crate::Item;
use crate::component::{ToolMaterial, common_item_components};
use mcrs_minecraft_block::tags as block_tags;
use mcrs_minecraft_registry::ItemId;
use mcrs_minecraft_registry::StaticRegistry;
use std::sync::LazyLock;

pub static ALL: &[&Item] = &[
    &TORCH,
    &WOODEN_PICKAXE,
    &STONE_PICKAXE,
    &GOLDEN_PICKAXE,
    &IRON_PICKAXE,
    &DIAMOND_PICKAXE,
    &IRON_AXE,
    &ELYTRA,
    &TRIDENT,
    &FISHING_ROD,
    &NAUTILUS_SHELL,
];

pub fn register_all_items(registry: &mut StaticRegistry<Item>) {
    for item in ALL {
        registry.register(item.identifier, *item);
    }
}

pub static TORCH: Item = Item {
    id: ItemId(395),
    identifier: mcrs_minecraft_core::rl!("minecraft:torch"),
    components: LazyLock::new(common_item_components),
};

pub static WOODEN_PICKAXE: Item = Item {
    id: ItemId(1027),
    identifier: mcrs_minecraft_core::rl!("minecraft:wooden_pickaxe"),
    components: LazyLock::new(|| ToolMaterial::WOOD.tool(block_tags::MINEABLE_PICKAXE)),
};

pub static STONE_PICKAXE: Item = Item {
    id: ItemId(1037),
    identifier: mcrs_minecraft_core::rl!("minecraft:stone_pickaxe"),
    components: LazyLock::new(|| ToolMaterial::STONE.tool(block_tags::MINEABLE_PICKAXE)),
};

pub static GOLDEN_PICKAXE: Item = Item {
    id: ItemId(1042),
    identifier: mcrs_minecraft_core::rl!("minecraft:golden_pickaxe"),
    components: LazyLock::new(|| ToolMaterial::GOLD.tool(block_tags::MINEABLE_PICKAXE)),
};

pub static IRON_PICKAXE: Item = Item {
    id: ItemId(1047),
    identifier: mcrs_minecraft_core::rl!("minecraft:iron_pickaxe"),
    components: LazyLock::new(|| ToolMaterial::IRON.tool(block_tags::MINEABLE_PICKAXE)),
};

pub static DIAMOND_PICKAXE: Item = Item {
    id: ItemId(1052),
    identifier: mcrs_minecraft_core::rl!("minecraft:diamond_pickaxe"),
    components: LazyLock::new(|| ToolMaterial::DIAMOND.tool(block_tags::MINEABLE_PICKAXE)),
};

pub static IRON_AXE: Item = Item {
    id: ItemId(1048),
    identifier: mcrs_minecraft_core::rl!("minecraft:iron_axe"),
    components: LazyLock::new(common_item_components),
};

pub static ELYTRA: Item = Item {
    id: ItemId(974),
    identifier: mcrs_minecraft_core::rl!("minecraft:elytra"),
    components: LazyLock::new(common_item_components),
};

pub static TRIDENT: Item = Item {
    id: ItemId(1483),
    identifier: mcrs_minecraft_core::rl!("minecraft:trident"),
    components: LazyLock::new(common_item_components),
};

pub static FISHING_ROD: Item = Item {
    id: ItemId(1186),
    identifier: mcrs_minecraft_core::rl!("minecraft:fishing_rod"),
    components: LazyLock::new(common_item_components),
};

pub static NAUTILUS_SHELL: Item = Item {
    id: ItemId(1484),
    identifier: mcrs_minecraft_core::rl!("minecraft:nautilus_shell"),
    components: LazyLock::new(common_item_components),
};

const STATE_TABLE_LEN: usize = 1 << 16;

static ID_TO_ITEM: [Option<&'static Item>; STATE_TABLE_LEN] = {
    let mut t: [Option<&'static Item>; STATE_TABLE_LEN] = [None; STATE_TABLE_LEN];
    let mut i = 0;
    while i < ALL.len() {
        t[ALL[i].id.0 as usize] = Some(ALL[i]);
        i += 1;
    }
    t
};

impl TryFrom<ItemId> for &'static Item {
    type Error = ();

    #[inline]
    fn try_from(v: ItemId) -> Result<Self, Self::Error> {
        ID_TO_ITEM.get(v.0 as usize).and_then(|x| *x).ok_or(())
    }
}

impl AsRef<Item> for ItemId {
    #[inline]
    fn as_ref(&self) -> &Item {
        ID_TO_ITEM[self.0 as usize].unwrap_or_else(|| panic!("Invalid item id: {}", self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::ALL;
    use mcrs_minecraft_registry::{RegistryLookup, StaticRegistryTable};

    #[test]
    fn item_ids_match_the_registry_report() {
        let table = StaticRegistryTable::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        ))
        .unwrap();
        for item in ALL {
            let reported = table.id("item", &item.identifier.to_arc());
            assert_eq!(reported, Some(u32::from(item.id.0)), "{}", item.identifier);
        }
    }
}
