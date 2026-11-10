use bevy_app::Update;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::Changed;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_item::Effective;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MaxDamage;

use crate::fixture::LORED;
use crate::fixture::PLAIN;
use crate::fixture::SWORD;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::lore;
use crate::fixture::spawn;
use crate::fixture::value;

#[derive(Component)]
struct Held;

#[derive(Resource, Default)]
struct Sent(Vec<ItemStack>);

fn wear(mut commands: Commands, mut tools: Query<(Entity, Effective<MaxDamage>, &mut Damage)>) {
    for (stack, max, mut damage) in &mut tools {
        let Some(max) = max.get() else { continue };
        damage.0 += 1;
        if damage.0 >= max.0 {
            commands.entity(stack).consume(1);
        }
    }
}

fn sign(mut commands: Commands, mut held: Query<(Entity, Option<&mut Lore>), With<Held>>) {
    for (stack, lore) in &mut held {
        match lore {
            Some(mut lore) => lore.lines.to_mut().push("signed".into()),
            None => {
                commands.entity(stack).set(Lore {
                    lines: vec!["signed".into()].into(),
                });
            }
        }
    }
}

fn send_slots(mut stacks: Stacks, changed: Query<Entity, Changed<Count>>, mut sent: ResMut<Sent>) {
    for stack in &changed {
        sent.0.extend(stacks.to_value(stack));
    }
}

#[test]
fn wear_breaks_the_tool_at_its_max_damage() {
    let mut app = app();
    app.add_systems(Update, wear);
    let sword = spawn(app.world_mut(), ItemStack::new(SWORD, count(1)));

    app.update();
    assert_eq!(app.world().get::<Damage>(sword), Some(&Damage(1)));
    app.update();
    app.update();
    assert!(app.world().get_entity(sword).is_err());
}

#[test]
fn consume_lowers_the_count_and_despawns_at_zero() {
    let mut app = app();
    let stack = spawn(app.world_mut(), ItemStack::new(PLAIN, count(5)));

    app.world_mut().commands().entity(stack).consume(2);
    app.world_mut().flush();
    assert_eq!(app.world().get::<Count>(stack), Some(&count(3)));

    app.world_mut().commands().entity(stack).consume(3);
    app.world_mut().flush();
    assert!(app.world().get_entity(stack).is_err());
}

#[test]
fn sign_appends_to_the_lore_or_sets_one() {
    let mut app = app();
    app.add_systems(Update, sign);
    let lored = spawn(app.world_mut(), ItemStack::new(LORED, count(1)));
    let plain = spawn(app.world_mut(), ItemStack::new(PLAIN, count(1)));
    app.world_mut().entity_mut(lored).insert(Held);
    app.world_mut().entity_mut(plain).insert(Held);

    app.update();

    let world = app.world_mut();
    assert_eq!(
        value(world, lored),
        Some(ItemStack::new(LORED, count(1)).set(lore(&["old", "signed"])))
    );
    assert_eq!(
        value(world, plain),
        Some(ItemStack::new(PLAIN, count(1)).set(lore(&["signed"])))
    );
}

#[test]
fn send_slots_sends_the_value_of_each_changed_count() {
    let mut app = app();
    app.init_resource::<Sent>().add_systems(Update, send_slots);
    let stack = spawn(app.world_mut(), ItemStack::new(PLAIN, count(2)));

    app.update();
    app.world_mut().commands().entity(stack).consume(1);
    app.update();
    app.update();

    let sent = &app.world().resource::<Sent>().0;
    assert_eq!(
        sent,
        &[
            ItemStack::new(PLAIN, count(2)),
            ItemStack::new(PLAIN, count(1))
        ]
    );
}
