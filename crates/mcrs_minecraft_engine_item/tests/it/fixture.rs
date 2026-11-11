use std::sync::LazyLock;

use bevy_app::App;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::SystemState;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomComponent;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_text::Text;

pub const PLAIN: Kind = Kind(0);
pub const SWORD: Kind = Kind(1);
pub const FOOD: Kind = Kind(2);
pub const LORED: Kind = Kind(3);

pub static TABLE: LazyLock<ItemTable> = LazyLock::new(|| {
    ItemTable::leak([
        vec![Builtin::MaxStackSize(MaxStackSize(64))],
        vec![
            Builtin::MaxStackSize(MaxStackSize(1)),
            Builtin::MaxDamage(MaxDamage(3)),
            Builtin::Damage(Damage(0)),
            Builtin::Rarity(Rarity::Uncommon),
            Builtin::Tool(Tool::new(Vec::new())),
        ],
        vec![
            Builtin::MaxStackSize(MaxStackSize(64)),
            Builtin::Food(Food::new(4, 0.3).unwrap()),
        ],
        vec![
            Builtin::MaxStackSize(MaxStackSize(64)),
            Builtin::Lore(lore(&["old"])),
        ],
    ])
});

#[derive(Component, Clone, Debug, PartialEq, Hash)]
pub struct Soulbound(pub u64);

impl CustomComponent for Soulbound {
    const KEY: &'static str = "example:soulbound";

    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.extend(self.0.to_le_bytes());
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        Some(Self(u64::from_le_bytes(bytes.try_into().ok()?)))
    }
}

/// A custom type that no world registers.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Unregistered(pub u8);

impl CustomComponent for Unregistered {
    const KEY: &'static str = "example:unregistered";

    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.0);
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [value] => Some(Self(*value)),
            _ => None,
        }
    }
}

pub fn lore(lines: &[&'static str]) -> Lore {
    Lore::new(
        lines
            .iter()
            .map(|line| Text::text(*line))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

pub fn count(count: u8) -> Count {
    Count::new(count).unwrap()
}

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins(ItemPlugin::new(*TABLE))
        .register_item_component::<Soulbound>()
        .unwrap();
    app
}

pub fn spawn(world: &mut World, value: ItemStack) -> Entity {
    let stack = world.commands().spawn_stack(value).id();
    world.flush();
    stack
}

pub fn value(world: &mut World, stack: Entity) -> Option<ItemStack> {
    SystemState::<Stacks>::new(world)
        .get_mut(world)
        .unwrap()
        .to_value(stack)
}
