use crate::tag::block::MINEABLE_PICKAXE;
use crate::world::item::Item;
use crate::world::item::component::ItemComponents;
use crate::world::item::component::tool::ToolMaterial;
use mcrs_protocol::{ItemId, ident};

pub const WOODEN_PICKAXE: Item = Item {
    id: ItemId(913),
    identifier: ident!("wooden_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::WOOD,
        1.0,
        -2.0,
        &ToolMaterial::WOOD.for_mineable_blocks(MINEABLE_PICKAXE),
    ),
};

pub const STONE_PICKAXE: Item = Item {
    id: ItemId(923),
    identifier: ident!("stone_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::STONE,
        1.0,
        -2.0,
        &ToolMaterial::STONE.for_mineable_blocks(MINEABLE_PICKAXE),
    ),
};

pub const GOLDEN_PICKAXE: Item = Item {
    id: ItemId(928),
    identifier: ident!("golden_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::GOLD,
        1.0,
        -2.8,
        &ToolMaterial::GOLD.for_mineable_blocks(MINEABLE_PICKAXE),
    ),
};

pub const IRON_PICKAXE: Item = Item {
    id: ItemId(933),
    identifier: ident!("iron_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::IRON,
        1.0,
        -2.8,
        &ToolMaterial::IRON.for_mineable_blocks(MINEABLE_PICKAXE),
    ),
};

pub const DIAMOND_PICKAXE: Item = Item {
    id: ItemId(938),
    identifier: ident!("diamond_pickaxe"),
    components: &ItemComponents::new().with_pickaxe(
        &ToolMaterial::DIAMOND,
        1.0,
        -2.8,
        &ToolMaterial::DIAMOND.for_mineable_blocks(MINEABLE_PICKAXE),
    ),
};

const STATE_TABLE_LEN: usize = 1 << 16;

// todo: macros
static ID_TO_ITEM: [Option<&'static Item>; STATE_TABLE_LEN] = {
    let mut t: [Option<&'static Item>; STATE_TABLE_LEN] = [None; STATE_TABLE_LEN];
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
        ID_TO_ITEM[self.0 as usize].expect(&format!("Invalid item id: {}", self.0))
    }
}
