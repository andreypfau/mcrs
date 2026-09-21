use bevy_ecs::entity::Entity;
use mcrs_minecraft_inventory::{Click, MenuSnapshot, Op, Planner, Slot, Source, StackKey, StackView, player_menu_layout};
use mcrs_minecraft_item::slots;
use mcrs_minecraft_protocol::item::{ComponentPatch, ContainerInput};
use mcrs_minecraft_registry::ItemId;

fn stone(count: u8) -> StackView {
    StackView {
        key: StackKey {
            item: ItemId(1),
            components: ComponentPatch::EMPTY,
        },
        count,
        max: 64,
        stackable: true,
        armour: None,
        offhand: false,
    }
}

fn helmet() -> StackView {
    StackView {
        key: StackKey {
            item: ItemId(2),
            components: ComponentPatch::EMPTY,
        },
        count: 1,
        max: 1,
        stackable: false,
        armour: Some(slots::ARMOR_HEAD),
        offhand: false,
    }
}

fn player() -> Entity {
    Entity::from_raw_u32(7).unwrap()
}

fn fresh() -> MenuSnapshot {
    MenuSnapshot::empty(player(), 3, player_menu_layout(player()))
}

fn cell(index: u16) -> Slot {
    Slot::new(player(), index)
}

fn click(snapshot: &mut MenuSnapshot, input: ContainerInput, slot: i16, button: u8) -> Vec<Op> {
    let mut planner = Planner::new(snapshot);
    planner.click(Click {
        slot,
        button,
        input,
        creative: false,
    });
    planner.ops
}

#[test]
fn left_click_lifts_the_stack_and_right_click_takes_half_then_places_one() {
    let mut snapshot = fresh();
    snapshot.set(cell(36), Some(stone(7)));
    let ops = click(&mut snapshot, ContainerInput::Pickup, 36, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: cell(36),
            to: cell(slots::CARRIED),
            count: 7
        }]
    );
    assert_eq!(snapshot.get(cell(36)), None);
    assert_eq!(snapshot.count(cell(slots::CARRIED)), 7);

    let ops = click(&mut snapshot, ContainerInput::Pickup, 9, 1);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: cell(slots::CARRIED),
            to: cell(9),
            count: 1
        }]
    );
    assert_eq!((snapshot.count(cell(9)), snapshot.count(cell(slots::CARRIED))), (1, 6));

    let ops = click(&mut snapshot, ContainerInput::Pickup, 9, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: cell(slots::CARRIED),
            to: cell(9),
            count: 6
        }]
    );
    let mut snapshot = fresh();
    snapshot.set(cell(36), Some(stone(7)));
    click(&mut snapshot, ContainerInput::Pickup, 36, 1);
    assert_eq!((snapshot.count(cell(36)), snapshot.count(cell(slots::CARRIED))), (3, 4));
}

#[test]
fn shift_click_merges_then_fills_and_a_helmet_goes_to_its_armour_cell() {
    let mut snapshot = fresh();
    snapshot.set(cell(36), Some(stone(40)));
    snapshot.set(cell(9), Some(stone(60)));
    let ops = click(&mut snapshot, ContainerInput::QuickMove, 36, 0);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: cell(36),
                to: cell(9),
                count: 4
            },
            Op::Transfer {
                from: cell(36),
                to: cell(10),
                count: 36
            },
        ]
    );
    let mut snapshot = fresh();
    snapshot.set(cell(36), Some(helmet()));
    let ops = click(&mut snapshot, ContainerInput::QuickMove, 36, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: cell(36),
            to: cell(slots::ARMOR_HEAD),
            count: 1
        }]
    );
}

#[test]
fn placing_a_stack_on_a_different_one_swaps_and_the_result_cell_refuses() {
    let mut snapshot = fresh();
    snapshot.set(cell(slots::CARRIED), Some(stone(5)));
    snapshot.set(cell(10), Some(helmet()));
    let ops = click(&mut snapshot, ContainerInput::Pickup, 10, 0);
    assert_eq!(
        ops,
        [Op::Swap {
            a: cell(10),
            b: cell(slots::CARRIED)
        }]
    );
    assert_eq!(snapshot.get(cell(10)), Some(&stone(5)));
    let ops = click(&mut snapshot, ContainerInput::Pickup, slots::RESULT as i16, 0);
    assert!(ops.is_empty(), "{ops:?}");
}

#[test]
fn a_stack_of_pumpkins_swapped_onto_a_helmet_splits_one_and_stores_the_helmet() {
    let pumpkins = StackView {
        armour: Some(slots::ARMOR_HEAD),
        ..stone(5)
    };
    let mut snapshot = fresh();
    snapshot.set(cell(slots::held(0)), Some(pumpkins));
    snapshot.set(cell(slots::ARMOR_HEAD), Some(helmet()));
    let ops = click(&mut snapshot, ContainerInput::Swap, slots::ARMOR_HEAD as i16, 0);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: cell(slots::ARMOR_HEAD),
                to: cell(slots::held(1)),
                count: 1
            },
            Op::Transfer {
                from: cell(slots::held(0)),
                to: cell(slots::ARMOR_HEAD),
                count: 1
            },
        ]
    );
    assert_eq!(snapshot.count(cell(slots::held(0))), 4);
}

#[test]
fn throwing_outside_drops_the_cursor_and_close_returns_it_first_to_the_held_slot() {
    let mut snapshot = fresh();
    snapshot.set(cell(slots::CARRIED), Some(stone(10)));
    let ops = click(&mut snapshot, ContainerInput::Pickup, -999, 1);
    assert_eq!(
        ops,
        [Op::Drop {
            from: cell(slots::CARRIED),
            count: 1,
            thrower: player()
        }]
    );
    snapshot.set(cell(slots::held(3)), Some(stone(60)));
    snapshot.set(cell(36), Some(stone(50)));
    let mut planner = Planner::new(&mut snapshot);
    planner.close();
    assert_eq!(
        planner.ops,
        [
            Op::Transfer {
                from: cell(slots::CARRIED),
                to: cell(slots::held(3)),
                count: 4
            },
            Op::Transfer {
                from: cell(slots::CARRIED),
                to: cell(36),
                count: 5
            },
        ]
    );
}

#[test]
fn a_full_inventory_drops_what_it_cannot_take() {
    let mut snapshot = fresh();
    for index in slots::HOTBAR.chain(slots::MAIN) {
        snapshot.set(cell(index), Some(stone(64)));
    }
    snapshot.set(cell(slots::CARRIED), Some(helmet()));
    let mut planner = Planner::new(&mut snapshot);
    planner.close();
    assert_eq!(
        planner.ops,
        [Op::Drop {
            from: cell(slots::CARRIED),
            count: 1,
            thrower: player()
        }]
    );
}

#[test]
fn a_pickup_fills_the_held_slot_first_then_a_free_cell() {
    let mut snapshot = fresh();
    for index in slots::HOTBAR.chain(slots::MAIN) {
        let count = if index == slots::held(3) { 60 } else { 64 };
        snapshot.set(cell(index), Some(stone(count)));
    }
    snapshot.set(cell(slots::MAIN.start), None);
    let item = Entity::from_raw_u32(99).unwrap();
    snapshot.add_item(item, stone(7));
    let mut planner = Planner::new(&mut snapshot);
    assert_eq!(planner.room_for(&stone(7)), 4 + 64);
    planner.insert_stack(Source::Item(item));
    assert_eq!(
        planner.ops,
        [
            Op::Pickup {
                item,
                to: cell(slots::held(3)),
                count: 4
            },
            Op::Pickup {
                item,
                to: cell(slots::MAIN.start),
                count: 3
            },
        ]
    );
}
