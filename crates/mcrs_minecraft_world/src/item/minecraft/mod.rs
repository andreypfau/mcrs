use crate::block::tags as block_tags;
use crate::item::Item;
use crate::item::component::ItemComponents;
use crate::item::component::tool::ToolMaterial;
use mcrs_minecraft_core::StaticRegistry;
use mcrs_minecraft_protocol::ItemId;

pub fn register_all_items(registry: &mut StaticRegistry<Item>) {
    let items: &[&'static Item] = &[
        &TORCH,
        &WOODEN_PICKAXE,
        &STONE_PICKAXE,
        &GOLDEN_PICKAXE,
        &IRON_PICKAXE,
        &DIAMOND_PICKAXE,
    ];
    for item in items {
        registry.register(item.identifier, *item);
    }
}

pub const TORCH: Item = Item {
    id: ItemId(323),
    identifier: mcrs_minecraft_core::rl!("minecraft:torch"),
    components: &ItemComponents::new(),
};

pub const WOODEN_PICKAXE: Item = Item {
    id: ItemId(914),
    identifier: mcrs_minecraft_core::rl!("minecraft:wooden_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::WOOD,
        &ToolMaterial::WOOD.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const STONE_PICKAXE: Item = Item {
    id: ItemId(924),
    identifier: mcrs_minecraft_core::rl!("minecraft:stone_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::STONE,
        &ToolMaterial::STONE.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const GOLDEN_PICKAXE: Item = Item {
    id: ItemId(929),
    identifier: mcrs_minecraft_core::rl!("minecraft:golden_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::GOLD,
        &ToolMaterial::GOLD.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const IRON_PICKAXE: Item = Item {
    id: ItemId(934),
    identifier: mcrs_minecraft_core::rl!("minecraft:iron_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::IRON,
        &ToolMaterial::IRON.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const DIAMOND_PICKAXE: Item = Item {
    id: ItemId(939),
    identifier: mcrs_minecraft_core::rl!("minecraft:diamond_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::DIAMOND,
        &ToolMaterial::DIAMOND.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

const STATE_TABLE_LEN: usize = 1 << 16;

// todo: macros
static ID_TO_ITEM: [Option<&'static Item>; STATE_TABLE_LEN] = {
    let mut t: [Option<&'static Item>; STATE_TABLE_LEN] = [None; STATE_TABLE_LEN];
    t[TORCH.id.0 as usize] = Some(&TORCH);
    t[WOODEN_PICKAXE.id.0 as usize] = Some(&WOODEN_PICKAXE);
    t[STONE_PICKAXE.id.0 as usize] = Some(&STONE_PICKAXE);
    t[GOLDEN_PICKAXE.id.0 as usize] = Some(&GOLDEN_PICKAXE);
    t[IRON_PICKAXE.id.0 as usize] = Some(&IRON_PICKAXE);
    t[DIAMOND_PICKAXE.id.0 as usize] = Some(&DIAMOND_PICKAXE);
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
