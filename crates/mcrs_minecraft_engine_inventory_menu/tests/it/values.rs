use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_inventory_menu::MAX_PARTS;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_menu::MenuEntry;

#[test]
fn parts_hold_at_most_max_parts_and_name_only_existing_parts() {
    let mut world = World::new();
    let holders: Vec<Entity> = (0..=MAX_PARTS).map(|_| world.spawn_empty().id()).collect();
    assert_eq!(MenuParts::new(holders.iter().copied()), None);

    let parts = MenuParts::new(holders.iter().copied().take(2)).unwrap();
    let cases = [
        (
            MenuEntry { part: 0, index: 5 },
            Some(SlotRef {
                holder: holders[0],
                index: 5,
            }),
        ),
        (
            MenuEntry { part: 1, index: 0 },
            Some(SlotRef {
                holder: holders[1],
                index: 0,
            }),
        ),
        (MenuEntry { part: 2, index: 0 }, None),
        (MenuEntry { part: 4, index: 0 }, None),
    ];
    for (entry, slot) in cases {
        assert_eq!(parts.slot(entry), slot, "{entry:?}");
    }
}

#[test]
fn buttons_hold_one_bit_for_each_entry_below_the_layout_limit() {
    let buttons = MenuButtons::default().with(0).with(127).with(128);
    let cases = [
        (0, true),
        (1, false),
        (127, true),
        (128, false),
        (u16::MAX, false),
    ];
    for (entry, pressed) in cases {
        assert_eq!(buttons.contains(entry), pressed, "{entry}");
    }
    assert_eq!(buttons, MenuButtons::default().with(0).with(127));
}
