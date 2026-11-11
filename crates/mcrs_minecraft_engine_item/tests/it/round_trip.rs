use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Tool;

use crate::fixture::FOOD;
use crate::fixture::LORED;
use crate::fixture::PLAIN;
use crate::fixture::SWORD;
use crate::fixture::Soulbound;
use crate::fixture::Unregistered;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::lore;
use crate::fixture::spawn;
use crate::fixture::value;

#[test]
fn to_value_gives_back_the_spawned_stack() {
    let stacks = [
        ItemStack::new(PLAIN, count(64)),
        ItemStack::new(SWORD, count(1))
            .set(Damage(2))
            .set(Rarity::Epic),
        ItemStack::new(SWORD, count(1))
            .unset::<Rarity>()
            .unset::<Tool>(),
        ItemStack::new(LORED, count(3)).unset::<Lore>(),
        ItemStack::new(FOOD, count(7))
            .set(lore(&["new"]))
            .set(RepairCost(4)),
        ItemStack::new(PLAIN, count(1)).set(Soulbound(7)),
        ItemStack::new(PLAIN, count(1)).set(Unregistered(9)),
    ];
    let mut app = app();
    for stack in stacks {
        let world = app.world_mut();
        let entity = spawn(world, stack.clone());
        assert_eq!(value(world, entity), Some(stack));
    }
}
