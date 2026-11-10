use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::SystemState;
use mcrs_minecraft_engine_item::ComponentState;
use mcrs_minecraft_engine_item::Effective;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_table::ItemTable;

use crate::fixture::PLAIN;
use crate::fixture::SWORD;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::spawn;
use crate::fixture::value;

#[test]
fn effective_state_follows_the_prototype_and_the_patch() {
    let cases = [
        (
            ItemStack::new(SWORD, count(1)),
            ComponentState::Default(&Rarity::Uncommon),
            ComponentState::Default(&MaxDamage(3)),
        ),
        (
            ItemStack::new(SWORD, count(1))
                .set(Rarity::Epic)
                .set(MaxDamage(5)),
            ComponentState::Overridden(&Rarity::Epic),
            ComponentState::Overridden(&MaxDamage(5)),
        ),
        (
            ItemStack::new(SWORD, count(1))
                .unset::<Rarity>()
                .unset::<MaxDamage>(),
            ComponentState::Removed,
            ComponentState::Removed,
        ),
        (
            ItemStack::new(PLAIN, count(1)),
            ComponentState::Default(&Rarity::Common),
            ComponentState::Absent,
        ),
    ];
    let mut app = app();
    for (stack, rarity, max_damage) in cases {
        let world = app.world_mut();
        let entity = spawn(world, stack.clone());
        let mut state = SystemState::<(
            Query<(Effective<Rarity>, Effective<MaxDamage>)>,
            Res<ItemTable>,
            Stacks,
        )>::new(world);
        let (query, table, mut stacks) = state.get_mut(world).unwrap();
        let (effective_rarity, effective_max) = query.get(entity).unwrap();

        assert_eq!(effective_rarity.state(&table), rarity, "{stack:?}");
        assert_eq!(effective_max.state(&table), max_damage, "{stack:?}");
        assert_eq!(stacks.state::<Rarity>(entity), rarity, "{stack:?}");
        assert_eq!(stacks.state::<MaxDamage>(entity), max_damage, "{stack:?}");
    }
}

#[test]
fn reset_gives_back_the_prototype_components() {
    let mut app = app();
    let world = app.world_mut();
    let patched = ItemStack::new(SWORD, count(1))
        .unset::<Rarity>()
        .set(MaxDamage(5));
    let entity = spawn(world, patched);
    world
        .commands()
        .entity(entity)
        .reset::<Rarity>()
        .reset::<MaxDamage>();
    world.flush();

    assert_eq!(value(world, entity), Some(ItemStack::new(SWORD, count(1))));
}
