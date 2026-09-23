mod common;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use common::{holder, items, place, value, world};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_inventory::value::spawn_stack;
use mcrs_minecraft_inventory::{
    MenuSnapshot, Slot, StackView, container_menu_layout, menu_slots, player_menu_layout,
};
use mcrs_minecraft_item::enchantment::test_enchantments;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_protocol::item::{ComponentPatch, Enchantments};

fn enchanted_chestplate(world: &mut World, enchantment: &str) -> Entity {
    let mut chestplate = value("iron_chestplate", 1, ComponentPatch::EMPTY);
    chestplate.components.set(Enchantments(vec![(
        ResourceKey::from_location(ResourceLocation::parse(enchantment).unwrap()),
        1,
    )]));
    spawn_stack(world, &chestplate, items()).unwrap()
}

#[test]
fn only_an_enchantment_preventing_armour_change_marks_the_stack_binding() {
    let mut world = world();
    world.insert_resource(test_enchantments().clone());
    let player = holder(&mut world, slots::COUNT);
    let cursed = enchanted_chestplate(&mut world, "minecraft:binding_curse");
    let unbreaking = enchanted_chestplate(&mut world, "minecraft:unbreaking");
    let plain = common::spawn(&mut world, "iron_chestplate", 1);
    place(&mut world, cursed, player, slots::ARMOR_CHEST).unwrap();
    place(&mut world, unbreaking, player, slots::MAIN.start).unwrap();
    place(&mut world, plain, player, slots::MAIN.start + 1).unwrap();

    let snapshot = MenuSnapshot::new(&world, items(), player, player_menu_layout(player));
    let binding = |index| {
        snapshot
            .get(Slot::new(player, index))
            .unwrap()
            .binding_curse
    };

    assert!(binding(slots::ARMOR_CHEST));
    assert!(!binding(slots::MAIN.start));
    assert!(!binding(slots::MAIN.start + 1));
}

#[test]
fn an_open_shulker_box_refuses_a_shulker_box_and_accepts_a_bundle() {
    let mut world = world();
    world.insert_resource(common::item_tags());
    let player = holder(&mut world, slots::COUNT);
    let container = holder(&mut world, 27);
    let nested = common::spawn(&mut world, "shulker_box", 1);
    let bundle = common::spawn(&mut world, "bundle", 1);
    let nested = StackView::of(&world, nested, items()).unwrap();
    let bundle = StackView::of(&world, bundle, items()).unwrap();

    assert!(!nested.fits_inside_container_items);
    assert!(bundle.fits_inside_container_items);

    let mut in_shulker = MenuSnapshot::new(
        &world,
        items(),
        player,
        container_menu_layout(
            container,
            player,
            menu_slots("minecraft:shulker_box").unwrap(),
        ),
    );
    in_shulker.shulker_box_slots = true;
    assert_eq!(in_shulker.slot_max(Slot::new(container, 13), &nested), None);
    assert_eq!(
        in_shulker.slot_max(Slot::new(container, 13), &bundle),
        Some(1)
    );
    assert_eq!(
        in_shulker.slot_max(Slot::new(player, slots::MAIN.start), &nested),
        Some(1)
    );

    let in_chest = MenuSnapshot::new(
        &world,
        items(),
        player,
        container_menu_layout(
            container,
            player,
            menu_slots("minecraft:generic_9x3").unwrap(),
        ),
    );
    assert_eq!(
        in_chest.slot_max(Slot::new(container, 13), &nested),
        Some(1)
    );
    assert_eq!(
        in_chest.slot_max(Slot::new(container, 13), &bundle),
        Some(1)
    );
}
