use bevy_app::Update;
use bevy_ecs::component::Component;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_ecs::system::SystemState;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::Rarity;

use crate::fixture::LORED;
use crate::fixture::SWORD;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::lore;
use crate::fixture::spawn;

#[derive(Component)]
struct InPlace;

fn append_lore(mut lores: Query<&mut Lore, With<InPlace>>) {
    for mut lore in &mut lores {
        lore.lines.to_mut().push("signed".into());
    }
}

fn make_common(mut rarities: Query<&mut Rarity, With<InPlace>>) {
    for mut rarity in &mut rarities {
        *rarity = Rarity::Common;
    }
}

#[test]
fn a_write_in_place_equals_the_set_command() {
    let mut app = app();
    app.add_systems(Update, (append_lore, make_common));
    let world = app.world_mut();
    let in_place = [
        spawn(world, ItemStack::new(LORED, count(1))),
        spawn(world, ItemStack::new(SWORD, count(1))),
    ];
    let by_command = [
        spawn(world, ItemStack::new(LORED, count(1))),
        spawn(world, ItemStack::new(SWORD, count(1))),
    ];
    for stack in in_place {
        world.entity_mut(stack).insert(InPlace);
    }
    let mut commands = world.commands();
    commands.entity(by_command[0]).set(lore(&["old", "signed"]));
    commands.entity(by_command[1]).set(Rarity::Common);

    app.update();

    let world = app.world_mut();
    let mut state = SystemState::<Stacks>::new(world);
    let mut stacks = state.get_mut(world).unwrap();
    for (left, right) in in_place.into_iter().zip(by_command) {
        assert!(stacks.mergeable(left, right));
        assert_eq!(stacks.row_hash(left), stacks.row_hash(right));
        assert_eq!(stacks.to_value(left), stacks.to_value(right));
    }
}
