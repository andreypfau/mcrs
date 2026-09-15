use crate::Item;
use crate::component::ItemComponents;
use crate::component::tool::ToolMaterial;
use mcrs_minecraft_block::tags as block_tags;
use mcrs_minecraft_registry::ItemId;
use mcrs_minecraft_registry::StaticRegistry;

pub const ALL: &[&Item] = &[
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

pub const TORCH: Item = Item {
    id: ItemId(395),
    identifier: mcrs_minecraft_core::rl!("minecraft:torch"),
    components: &ItemComponents::new(),
};

pub const WOODEN_PICKAXE: Item = Item {
    id: ItemId(1027),
    identifier: mcrs_minecraft_core::rl!("minecraft:wooden_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::WOOD,
        &ToolMaterial::WOOD.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const STONE_PICKAXE: Item = Item {
    id: ItemId(1037),
    identifier: mcrs_minecraft_core::rl!("minecraft:stone_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::STONE,
        &ToolMaterial::STONE.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const GOLDEN_PICKAXE: Item = Item {
    id: ItemId(1042),
    identifier: mcrs_minecraft_core::rl!("minecraft:golden_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::GOLD,
        &ToolMaterial::GOLD.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const IRON_PICKAXE: Item = Item {
    id: ItemId(1047),
    identifier: mcrs_minecraft_core::rl!("minecraft:iron_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::IRON,
        &ToolMaterial::IRON.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const DIAMOND_PICKAXE: Item = Item {
    id: ItemId(1052),
    identifier: mcrs_minecraft_core::rl!("minecraft:diamond_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::DIAMOND,
        &ToolMaterial::DIAMOND.for_mineable_blocks(block_tags::MINEABLE_PICKAXE),
    ),
};

pub const IRON_AXE: Item = Item {
    id: ItemId(1048),
    identifier: mcrs_minecraft_core::rl!("minecraft:iron_axe"),
    components: &ItemComponents::new(),
};

pub const ELYTRA: Item = Item {
    id: ItemId(974),
    identifier: mcrs_minecraft_core::rl!("minecraft:elytra"),
    components: &ItemComponents::new(),
};

pub const TRIDENT: Item = Item {
    id: ItemId(1483),
    identifier: mcrs_minecraft_core::rl!("minecraft:trident"),
    components: &ItemComponents::new(),
};

pub const FISHING_ROD: Item = Item {
    id: ItemId(1186),
    identifier: mcrs_minecraft_core::rl!("minecraft:fishing_rod"),
    components: &ItemComponents::new(),
};

pub const NAUTILUS_SHELL: Item = Item {
    id: ItemId(1484),
    identifier: mcrs_minecraft_core::rl!("minecraft:nautilus_shell"),
    components: &ItemComponents::new(),
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
