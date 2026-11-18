use bevy::prelude::*;
use mcrs_minecraft_client::test_support::ServerAnswers;
use mcrs_minecraft_client::test_support::item_kind;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::StoredEnchantments;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Stone,
    Dirt,
    EnchantedBook,
}

const ITEMS: [Item; 3] = [Item::Stone, Item::Dirt, Item::EnchantedBook];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stack {
    pub(crate) item: Item,
    pub(crate) count: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub(crate) entry: u16,
    pub(crate) stack: Option<Stack>,
}

pub(crate) const fn held(item: Item, count: u8) -> Option<Stack> {
    Some(Stack { item, count })
}

pub(crate) const fn at(entry: u16, item: Item, count: u8) -> Entry {
    Entry {
        entry,
        stack: held(item, count),
    }
}

pub(crate) const fn empty(entry: u16) -> Entry {
    Entry { entry, stack: None }
}

pub(crate) const fn holding(entry: u16, stack: Option<Stack>) -> Entry {
    Entry { entry, stack }
}

pub(crate) const fn click(kind: ClickKind, button: u8, clicked: Clicked) -> Click {
    Click {
        kind,
        button,
        clicked,
    }
}

pub(crate) const fn on(kind: ClickKind, button: u8, entry: u16) -> Click {
    click(kind, button, Clicked::Entry(entry))
}

pub(crate) const AGREED: ServerAnswers = ServerAnswers {
    slots: 0,
    cursors: 0,
    contents: 0,
    player_slots: 0,
};

pub(crate) fn item_name(item: Item) -> &'static str {
    match item {
        Item::Stone => "minecraft:stone",
        Item::Dirt => "minecraft:dirt",
        Item::EnchantedBook => "minecraft:enchanted_book",
    }
}

pub(crate) fn kind(world: &World, item: Item) -> Kind {
    item_kind(world, item_name(item))
}

pub(crate) fn value(world: &World, stack: Option<Stack>) -> Option<ItemStack> {
    let stack = stack?;
    let value = ItemStack::new(kind(world, stack.item), Count::new(stack.count).unwrap());
    Some(match stack.item {
        Item::EnchantedBook => value.set(stored_sharpness(world)),
        Item::Stone | Item::Dirt => value,
    })
}

pub(crate) fn stored_sharpness(world: &World) -> StoredEnchantments {
    let names = RegistryNames::new(
        world.resource::<CurrentRegistries>(),
        world.resource::<ItemTable>(),
    );
    let sharpness = ResourceLocation::read("minecraft:sharpness").unwrap();
    let enchantment = EnchantmentId(names.id(NameTable::Enchantment, sharpness).unwrap());
    let level = EnchantmentLevel {
        enchantment,
        level: 5.try_into().unwrap(),
    };
    StoredEnchantments(Enchantments::new([level]).unwrap())
}

pub(crate) fn item_of(world: &World, found: Kind) -> Item {
    ITEMS
        .into_iter()
        .find(|item| kind(world, *item) == found)
        .expect("the test gives only stone, dirt and an enchanted book")
}

pub(crate) fn stack_at(world: &World, holder: Entity, index: u16) -> Option<Stack> {
    let stack = world.entity(world.get::<Slots>(holder)?.get(usize::from(index))?);
    let item = item_of(world, *stack.get::<Kind>()?);
    if item == Item::EnchantedBook {
        assert_eq!(
            stack.get::<StoredEnchantments>(),
            Some(&stored_sharpness(world)),
            "the book keeps its stored enchantments"
        );
    }
    Some(Stack {
        item,
        count: stack.get::<Count>()?.get(),
    })
}

pub(crate) fn cursor(world: &World, viewer: Entity) -> Option<Stack> {
    let holders = world.get::<PlayerHolders>(viewer)?;
    stack_at(world, holders.cursor, 0)
}
