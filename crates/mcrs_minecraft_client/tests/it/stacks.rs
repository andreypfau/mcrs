use bevy::prelude::*;
use mcrs_minecraft_client::test_support::ServerAnswers;
use mcrs_minecraft_client::test_support::item_kind;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_player_holders::PlayerHolders;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Stone,
    Dirt,
}

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
    }
}

pub(crate) fn kind(world: &World, item: Item) -> Kind {
    item_kind(world, item_name(item))
}

pub(crate) fn value(world: &World, stack: Option<Stack>) -> Option<ItemStack> {
    let stack = stack?;
    Some(ItemStack::new(
        kind(world, stack.item),
        Count::new(stack.count).unwrap(),
    ))
}

pub(crate) fn stack_at(world: &World, holder: Entity, index: u16) -> Option<Stack> {
    let stack = world.entity(world.get::<Slots>(holder)?.get(usize::from(index))?);
    let found = *stack.get::<Kind>()?;
    let item = [Item::Stone, Item::Dirt]
        .into_iter()
        .find(|item| kind(world, *item) == found)
        .expect("the test gives only stone and dirt");
    Some(Stack {
        item,
        count: stack.get::<Count>()?.get(),
    })
}

pub(crate) fn cursor(world: &World, viewer: Entity) -> Option<Stack> {
    let holders = world.get::<PlayerHolders>(viewer)?;
    stack_at(world, holders.cursor, 0)
}
