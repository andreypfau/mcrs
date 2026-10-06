use crate::common::{holder, items, place, value, world};
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_inventory::value::spawn_stack;
use mcrs_minecraft_inventory::{
    Click, MenuSnapshot, Planner, Slot, StackView, container_menu_layout, menu_slots,
    player_menu_layout,
};
use mcrs_minecraft_item::slots;
use mcrs_minecraft_keys::menu;
use mcrs_minecraft_protocol::item::{ComponentPatch, ContainerInput, Enchantments, Equippable};
use mcrs_minecraft_registry::DenseId;
use mcrs_minecraft_registry::{HolderSet, Id};
use mcrs_minecraft_world::item::{test_enchantment_effects, test_enchantment_registry};
use mcrs_minecraft_world::registries::test_registries;

fn enchanted_chestplate(world: &mut World, enchantment: &str) -> Entity {
    let mut chestplate = value("iron_chestplate", 1, ComponentPatch::EMPTY);
    chestplate.components.set(Enchantments(vec![(
        ResourceKey::from_location(ResourceLocation::read(enchantment).unwrap()),
        1,
    )]));
    spawn_stack(world, &chestplate, items()).unwrap()
}

#[test]
fn menu_snapshots_read_what_the_player_may_do() {
    only_an_enchantment_preventing_armour_change_marks_the_stack_binding();
    a_helmet_the_player_may_not_wear_keeps_its_slot_but_is_not_wearable();
    an_open_shulker_box_refuses_a_shulker_box_and_accepts_a_bundle();
}

fn only_an_enchantment_preventing_armour_change_marks_the_stack_binding() {
    let mut world = world();
    world.insert_resource(test_enchantment_registry());
    world.insert_resource(test_enchantment_effects());
    let player = holder(&mut world, slots::COUNT);
    let cursed = enchanted_chestplate(&mut world, "minecraft:binding_curse");
    let unbreaking = enchanted_chestplate(&mut world, "minecraft:unbreaking");
    let plain = crate::common::spawn(&mut world, "iron_chestplate", 1);
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

fn a_helmet_the_player_may_not_wear_keeps_its_slot_but_is_not_wearable() {
    let mut world = world();
    let registries = test_registries();
    world.insert_resource(registries.clone());
    let zombie = EntityType::Zombie;
    let player = holder(&mut world, slots::COUNT);
    let zombie_only = crate::common::spawn(&mut world, "iron_helmet", 1);
    world
        .get_mut::<Equippable>(zombie_only)
        .unwrap()
        .allowed_entities = Some(HolderSet::List(Box::new([zombie.id()])));
    let plain = crate::common::spawn(&mut world, "iron_helmet", 1);
    place(&mut world, zombie_only, player, slots::MAIN.start).unwrap();
    let mut snapshot = MenuSnapshot::new(&world, items(), player, player_menu_layout(player));
    let head = Slot::new(player, slots::ARMOR_HEAD);

    let zombie_only = StackView::of(&world, zombie_only, items()).unwrap();
    assert_eq!(zombie_only.armour, Some(slots::ARMOR_HEAD));
    assert!(!zombie_only.wearable);
    assert_eq!(snapshot.slot_max(head, &zombie_only), None);

    let mut planner = Planner::new(&mut snapshot);
    planner.click(Click {
        slot: slots::MAIN.start as i16,
        button: 0,
        input: ContainerInput::QuickMove,
        creative: false,
    });
    assert_eq!(planner.ops, []);

    let plain = StackView::of(&world, plain, items()).unwrap();
    assert_eq!(plain.armour, Some(slots::ARMOR_HEAD));
    assert!(plain.wearable);
    assert_eq!(snapshot.slot_max(head, &plain), Some(1));
}

fn an_open_shulker_box_refuses_a_shulker_box_and_accepts_a_bundle() {
    let mut world = world();
    world.insert_resource(crate::common::item_tags());
    let player = holder(&mut world, slots::COUNT);
    let container = holder(&mut world, 27);
    let nested = crate::common::spawn(&mut world, "shulker_box", 1);
    let bundle = crate::common::spawn(&mut world, "bundle", 1);
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
            menu_slots(menu::SHULKER_BOX.id()).unwrap(),
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
            menu_slots(menu::GENERIC_9X3.id()).unwrap(),
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

#[test]
fn every_menu_type_has_a_slot_layout_with_its_vanilla_slot_count() {
    let own: Vec<Option<u16>> = (0..menu::ENTRIES.len())
        .map(|number| menu_slots(Id::from_raw(number as u16)).map(|slots| slots.own))
        .collect();
    let expected: Vec<(&str, u16)> = vec![
        ("generic_9x1", 9),
        ("generic_9x2", 18),
        ("generic_9x3", 27),
        ("generic_9x4", 36),
        ("generic_9x5", 45),
        ("generic_9x6", 54),
        ("generic_3x3", 9),
        ("crafter_3x3", 9),
        ("anvil", 3),
        ("beacon", 1),
        ("blast_furnace", 3),
        ("brewing_stand", 5),
        ("crafting", 10),
        ("enchantment", 2),
        ("furnace", 3),
        ("grindstone", 3),
        ("hopper", 5),
        ("lectern", 1),
        ("loom", 4),
        ("merchant", 3),
        ("shulker_box", 27),
        ("smithing", 4),
        ("smoker", 3),
        ("cartography_table", 3),
        ("stonecutter", 2),
    ];
    assert_eq!(own.len(), expected.len());
    for (number, name) in menu::ENTRIES.iter().enumerate() {
        let (short, slots) = expected
            .iter()
            .find(|(short, _)| format!("minecraft:{short}") == name.as_static_str())
            .unwrap_or_else(|| panic!("{name} has no expected layout"));
        assert_eq!(own[number], Some(*slots), "{short}");
    }
    assert!(menu_slots(menu::LECTERN.id()).is_some_and(|slots| !slots.player_slots));
    assert!(menu_slots(menu::CRAFTER_3X3.id()).is_some_and(|slots| slots.trailing_result));
}
