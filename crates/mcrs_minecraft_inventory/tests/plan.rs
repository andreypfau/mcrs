use bevy_ecs::entity::Entity;
use mcrs_minecraft_inventory::{
    Click, Drag, Feed, MenuSnapshot, Op, Planner, SLOT_CLICKED_OUTSIDE, Slot, Source, StackKey,
    StackView, player_menu_layout, quick_craft_counts,
};
use mcrs_minecraft_item::slots;
use mcrs_minecraft_protocol::item::{
    ComponentPatch, ContainerInput, QuickCraftButton, QuickCraftKind, QuickCraftStage,
};
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
        binding_curse: false,
        fits_inside_container_items: true,
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
        binding_curse: false,
        fits_inside_container_items: true,
    }
}

fn leggings() -> StackView {
    StackView {
        key: StackKey {
            item: ItemId(3),
            components: ComponentPatch::EMPTY,
        },
        count: 1,
        max: 1,
        stackable: false,
        armour: Some(slots::ARMOR_LEGS),
        offhand: false,
        binding_curse: false,
        fits_inside_container_items: true,
    }
}

fn sword() -> StackView {
    StackView {
        key: StackKey {
            item: ItemId(4),
            components: ComponentPatch::EMPTY,
        },
        count: 1,
        max: 1,
        stackable: false,
        armour: None,
        offhand: false,
        binding_curse: false,
        fits_inside_container_items: true,
    }
}

fn chestplate() -> StackView {
    StackView {
        key: StackKey {
            item: ItemId(6),
            components: ComponentPatch::EMPTY,
        },
        count: 1,
        max: 1,
        stackable: false,
        armour: Some(slots::ARMOR_CHEST),
        offhand: false,
        binding_curse: false,
        fits_inside_container_items: true,
    }
}

fn cursed_chestplate() -> StackView {
    StackView {
        key: StackKey {
            item: ItemId(7),
            components: ComponentPatch::EMPTY,
        },
        count: 1,
        max: 1,
        stackable: false,
        armour: Some(slots::ARMOR_CHEST),
        offhand: false,
        binding_curse: true,
        fits_inside_container_items: true,
    }
}

fn player() -> Entity {
    Entity::from_raw_u32(7).unwrap()
}

fn fresh() -> MenuSnapshot {
    MenuSnapshot::empty(player(), 3, player_menu_layout(player()))
}

fn slot(index: u16) -> Slot {
    Slot::new(player(), index)
}

fn click(snapshot: &mut MenuSnapshot, input: ContainerInput, slot: i16, button: u8) -> Vec<Op> {
    click_as(snapshot, input, slot, button, false)
}

fn click_as(
    snapshot: &mut MenuSnapshot,
    input: ContainerInput,
    slot: i16,
    button: u8,
    creative: bool,
) -> Vec<Op> {
    let mut planner = Planner::new(snapshot);
    planner.click(Click {
        slot,
        button,
        input,
        creative,
    });
    planner.ops
}

fn drag(snapshot: &mut MenuSnapshot, kind: QuickCraftKind, indices: &[i16], creative: bool) -> Vec<Op> {
    let mut current: Option<Drag> = None;
    let feed = |current: &mut Option<Drag>, slot, stage, snapshot: &MenuSnapshot| {
        Drag::feed(
            current,
            Click {
                slot,
                button: u8::from(QuickCraftButton { kind, stage }),
                input: ContainerInput::QuickCraft,
                creative,
            },
            snapshot,
        )
    };
    feed(
        &mut current,
        SLOT_CLICKED_OUTSIDE,
        QuickCraftStage::Header,
        snapshot,
    );
    for &index in indices {
        feed(&mut current, index, QuickCraftStage::Slot, snapshot);
    }
    match feed(
        &mut current,
        SLOT_CLICKED_OUTSIDE,
        QuickCraftStage::End,
        snapshot,
    ) {
        Feed::Complete(drag) => {
            let mut planner = Planner::new(snapshot);
            planner.quick_craft(drag.kind, &drag.indices);
            planner.ops
        }
        Feed::Pending | Feed::Reset => Vec::new(),
    }
}

#[test]
fn left_click_lifts_the_stack_and_right_click_takes_half_then_places_one() {
    let mut snapshot = fresh();
    snapshot.set(slot(36), Some(stone(7)));
    let ops = click(&mut snapshot, ContainerInput::Pickup, 36, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(36),
            to: slot(slots::CARRIED),
            count: 7
        }]
    );
    assert_eq!(snapshot.get(slot(36)), None);
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 7);

    let ops = click(&mut snapshot, ContainerInput::Pickup, 9, 1);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(slots::CARRIED),
            to: slot(9),
            count: 1
        }]
    );
    assert_eq!(
        (
            snapshot.count(slot(9)),
            snapshot.count(slot(slots::CARRIED))
        ),
        (1, 6)
    );

    let ops = click(&mut snapshot, ContainerInput::Pickup, 9, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(slots::CARRIED),
            to: slot(9),
            count: 6
        }]
    );
    let mut snapshot = fresh();
    snapshot.set(slot(36), Some(stone(7)));
    click(&mut snapshot, ContainerInput::Pickup, 36, 1);
    assert_eq!(
        (
            snapshot.count(slot(36)),
            snapshot.count(slot(slots::CARRIED))
        ),
        (3, 4)
    );
}

#[test]
fn shift_click_merges_then_fills_and_a_helmet_goes_to_its_armour_slot() {
    let mut snapshot = fresh();
    snapshot.set(slot(36), Some(stone(40)));
    snapshot.set(slot(9), Some(stone(60)));
    let ops = click(&mut snapshot, ContainerInput::QuickMove, 36, 0);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(36),
                to: slot(9),
                count: 4
            },
            Op::Transfer {
                from: slot(36),
                to: slot(10),
                count: 36
            },
        ]
    );
    let mut snapshot = fresh();
    snapshot.set(slot(36), Some(helmet()));
    let ops = click(&mut snapshot, ContainerInput::QuickMove, 36, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(36),
            to: slot(slots::ARMOR_HEAD),
            count: 1
        }]
    );
}

#[test]
fn placing_a_stack_on_a_different_one_swaps_and_the_result_slot_refuses() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(5)));
    snapshot.set(slot(10), Some(helmet()));
    let ops = click(&mut snapshot, ContainerInput::Pickup, 10, 0);
    assert_eq!(
        ops,
        [Op::Swap {
            a: slot(10),
            b: slot(slots::CARRIED)
        }]
    );
    assert_eq!(snapshot.get(slot(10)), Some(&stone(5)));
    let ops = click(
        &mut snapshot,
        ContainerInput::Pickup,
        slots::RESULT as i16,
        0,
    );
    assert!(ops.is_empty(), "{ops:?}");
}

#[test]
fn a_stack_of_pumpkins_swapped_onto_a_helmet_splits_one_and_stores_the_helmet() {
    let pumpkins = StackView {
        armour: Some(slots::ARMOR_HEAD),
        ..stone(5)
    };
    let mut snapshot = fresh();
    snapshot.set(slot(slots::held(0)), Some(pumpkins));
    snapshot.set(slot(slots::ARMOR_HEAD), Some(helmet()));
    let ops = click(
        &mut snapshot,
        ContainerInput::Swap,
        slots::ARMOR_HEAD as i16,
        0,
    );
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::ARMOR_HEAD),
                to: slot(slots::held(1)),
                count: 1
            },
            Op::Transfer {
                from: slot(slots::held(0)),
                to: slot(slots::ARMOR_HEAD),
                count: 1
            },
        ]
    );
    assert_eq!(snapshot.count(slot(slots::held(0))), 4);
}

#[test]
fn throwing_outside_drops_the_cursor_and_close_returns_it_first_to_the_held_slot() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(10)));
    let ops = click(&mut snapshot, ContainerInput::Pickup, -999, 1);
    assert_eq!(
        ops,
        [Op::Drop {
            from: slot(slots::CARRIED),
            count: 1,
            thrower: player()
        }]
    );
    snapshot.set(slot(slots::held(3)), Some(stone(60)));
    snapshot.set(slot(36), Some(stone(50)));
    let mut planner = Planner::new(&mut snapshot);
    planner.close();
    assert_eq!(
        planner.ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(slots::held(3)),
                count: 4
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(36),
                count: 5
            },
        ]
    );
}

#[test]
fn a_full_inventory_drops_what_it_cannot_take() {
    let mut snapshot = fresh();
    for index in slots::HOTBAR.chain(slots::MAIN) {
        snapshot.set(slot(index), Some(stone(64)));
    }
    snapshot.set(slot(slots::CARRIED), Some(helmet()));
    let mut planner = Planner::new(&mut snapshot);
    planner.close();
    assert_eq!(
        planner.ops,
        [Op::Drop {
            from: slot(slots::CARRIED),
            count: 1,
            thrower: player()
        }]
    );
}

#[test]
fn left_drag_splits_64_over_five_empty_slots_into_12_each_and_leaves_4() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(64)));
    let indices: Vec<usize> = (9..14).collect();
    let (counts, remaining) = quick_craft_counts(QuickCraftKind::Split, &indices, &snapshot);
    assert_eq!(
        counts,
        [(9, 12), (10, 12), (11, 12), (12, 12), (13, 12)]
    );
    assert_eq!(remaining, 4);

    let ops = drag(&mut snapshot, QuickCraftKind::Split, &[9, 10, 11, 12, 13], false);
    assert_eq!(
        ops,
        (9..14)
            .map(|index| Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(index),
                count: 12
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 4);
}

#[test]
fn left_drag_tops_up_a_partial_stack_and_caps_at_the_stack_size() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(64)));
    snapshot.set(slot(9), Some(stone(60)));
    let ops = drag(&mut snapshot, QuickCraftKind::Split, &[9, 10], false);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 4
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 32
            },
        ]
    );
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 28);
}

#[test]
fn left_drag_of_a_16_max_item_caps_each_slot_at_16() {
    let capped = StackView { max: 16, ..stone(16) };
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(capped.clone()));
    snapshot.set(slot(9), Some(capped.with_count(14)));
    let ops = drag(&mut snapshot, QuickCraftKind::Split, &[9, 10, 11], false);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 2
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 5
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(11),
                count: 5
            },
        ]
    );
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 4);
}

#[test]
fn a_five_stack_over_six_slots_admits_only_five_and_empties_the_cursor() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(5)));
    let ops = drag(
        &mut snapshot,
        QuickCraftKind::Split,
        &[9, 10, 11, 12, 13, 14],
        false,
    );
    assert_eq!(
        ops,
        (9..14)
            .map(|index| Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(index),
                count: 1
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(snapshot.get(slot(slots::CARRIED)), None);
}

#[test]
fn a_drag_that_ends_on_one_slot_is_a_plain_click() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(7)));
    let mut expected = fresh();
    expected.set(slot(slots::CARRIED), Some(stone(7)));
    let expected_ops = click(&mut expected, ContainerInput::Pickup, 9, 0);

    let ops = drag(&mut snapshot, QuickCraftKind::Split, &[9], false);
    assert_eq!(ops, expected_ops);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(slots::CARRIED),
            to: slot(9),
            count: 7
        }]
    );
}

#[test]
fn duplicate_slot_packets_and_packet_order_do_not_change_the_result() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(64)));
    let ops = drag(&mut snapshot, QuickCraftKind::Split, &[9, 9, 10], false);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 32
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 32
            },
        ]
    );

    let mut reordered = fresh();
    reordered.set(slot(slots::CARRIED), Some(stone(64)));
    let ops = drag(&mut reordered, QuickCraftKind::Split, &[10, 9], false);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 32
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 32
            },
        ]
    );
    assert_eq!(snapshot.count(slot(9)), reordered.count(slot(9)));
    assert_eq!(snapshot.count(slot(10)), reordered.count(slot(10)));
}

#[test]
fn a_header_on_an_empty_cursor_and_an_end_with_no_slots_move_nothing() {
    let mut snapshot = fresh();
    let mut current: Option<Drag> = None;
    let feed = Drag::feed(
        &mut current,
        Click {
            slot: SLOT_CLICKED_OUTSIDE,
            button: u8::from(QuickCraftButton {
                kind: QuickCraftKind::Split,
                stage: QuickCraftStage::Header,
            }),
            input: ContainerInput::QuickCraft,
            creative: false,
        },
        &snapshot,
    );
    assert_eq!(feed, Feed::Reset);
    assert_eq!(current, None);

    snapshot.set(slot(slots::CARRIED), Some(stone(8)));
    let mut current: Option<Drag> = None;
    Drag::feed(
        &mut current,
        Click {
            slot: SLOT_CLICKED_OUTSIDE,
            button: u8::from(QuickCraftButton {
                kind: QuickCraftKind::Split,
                stage: QuickCraftStage::Header,
            }),
            input: ContainerInput::QuickCraft,
            creative: false,
        },
        &snapshot,
    );
    let feed = Drag::feed(
        &mut current,
        Click {
            slot: SLOT_CLICKED_OUTSIDE,
            button: u8::from(QuickCraftButton {
                kind: QuickCraftKind::Split,
                stage: QuickCraftStage::End,
            }),
            input: ContainerInput::QuickCraft,
            creative: false,
        },
        &snapshot,
    );
    assert_eq!(feed, Feed::Reset);
}

#[test]
fn a_full_kind_header_outside_creative_resets() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(8)));
    let mut current: Option<Drag> = None;
    let feed = Drag::feed(
        &mut current,
        Click {
            slot: SLOT_CLICKED_OUTSIDE,
            button: u8::from(QuickCraftButton {
                kind: QuickCraftKind::Full,
                stage: QuickCraftStage::Header,
            }),
            input: ContainerInput::QuickCraft,
            creative: false,
        },
        &snapshot,
    );
    assert_eq!(feed, Feed::Reset);
    assert_eq!(current, None);
}

#[test]
fn right_drag_places_one_in_each_slot() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(10)));
    snapshot.set(slot(10), Some(stone(3)));
    let indices: Vec<usize> = [9, 10, 11].to_vec();
    assert_eq!(
        quick_craft_counts(QuickCraftKind::Single, &indices, &snapshot),
        (vec![(9, 1), (10, 4), (11, 1)], 7)
    );
    let ops = drag(&mut snapshot, QuickCraftKind::Single, &[9, 10, 11], false);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 1
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 1
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(11),
                count: 1
            },
        ]
    );
    assert_eq!(snapshot.count(slot(9)), 1);
    assert_eq!(snapshot.count(slot(10)), 4);
    assert_eq!(snapshot.count(slot(11)), 1);
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 7);
}

#[test]
fn right_drag_with_exactly_as_many_items_as_slots_empties_the_cursor() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(3)));
    let ops = drag(&mut snapshot, QuickCraftKind::Single, &[9, 10, 11], false);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 1
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 1
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(11),
                count: 1
            },
        ]
    );
    assert_eq!(snapshot.get(slot(slots::CARRIED)), None);
}

#[test]
fn right_drag_ignores_a_repeated_slot() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(10)));
    let ops = drag(&mut snapshot, QuickCraftKind::Single, &[9, 9, 10], false);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 1
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 1
            },
        ]
    );
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 8);
}

#[test]
fn creative_middle_drag_fills_every_slot_and_empties_the_cursor() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(10)));
    let indices: Vec<usize> = [9, 10, 11].to_vec();
    assert_eq!(
        quick_craft_counts(QuickCraftKind::Full, &indices, &snapshot),
        (vec![(9, 64), (10, 64), (11, 64)], 0)
    );
    let ops = drag(&mut snapshot, QuickCraftKind::Full, &[9, 10, 11], true);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 10
            },
            Op::Clone {
                from: slot(9),
                to: slot(9)
            },
            Op::Clone {
                from: slot(9),
                to: slot(10)
            },
            Op::Clone {
                from: slot(9),
                to: slot(11)
            },
        ]
    );
    assert_eq!(snapshot.count(slot(9)), 64);
    assert_eq!(snapshot.count(slot(10)), 64);
    assert_eq!(snapshot.count(slot(11)), 64);
    assert_eq!(snapshot.get(slot(slots::CARRIED)), None);
}

#[test]
fn creative_middle_drag_tops_up_partial_stacks_from_the_cursor_first() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(64)));
    snapshot.set(slot(9), Some(stone(60)));
    snapshot.set(slot(10), Some(stone(60)));
    let indices: Vec<usize> = [9, 10].to_vec();
    assert_eq!(
        quick_craft_counts(QuickCraftKind::Full, &indices, &snapshot),
        (vec![(9, 64), (10, 64)], 56)
    );
    let ops = drag(&mut snapshot, QuickCraftKind::Full, &[9, 10], true);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 4
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 4
            },
        ]
    );
    assert_eq!(snapshot.count(slot(9)), 64);
    assert_eq!(snapshot.count(slot(10)), 64);
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 56);
}

#[test]
fn creative_middle_drag_whose_placed_sum_equals_the_cursor_ends_exactly_empty() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(64)));
    snapshot.set(slot(9), Some(stone(60)));
    snapshot.set(slot(10), Some(stone(4)));
    let indices: Vec<usize> = [9, 10].to_vec();
    assert_eq!(
        quick_craft_counts(QuickCraftKind::Full, &indices, &snapshot).1,
        0
    );
    let ops = drag(&mut snapshot, QuickCraftKind::Full, &[9, 10], true);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 4
            },
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(10),
                count: 60
            },
        ]
    );
    assert_eq!(snapshot.get(slot(slots::CARRIED)), None);
}

#[test]
fn creative_middle_drag_of_one_item_conjures_the_rest() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(1)));
    let indices: Vec<usize> = [9, 10, 11].to_vec();
    assert_eq!(
        quick_craft_counts(QuickCraftKind::Full, &indices, &snapshot).1,
        0
    );
    let ops = drag(&mut snapshot, QuickCraftKind::Full, &[9, 10, 11], true);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(slots::CARRIED),
                to: slot(9),
                count: 1
            },
            Op::Clone {
                from: slot(9),
                to: slot(9)
            },
            Op::Clone {
                from: slot(9),
                to: slot(10)
            },
            Op::Clone {
                from: slot(9),
                to: slot(11)
            },
        ]
    );
    assert_eq!(snapshot.count(slot(9)), 64);
    assert_eq!(snapshot.count(slot(10)), 64);
    assert_eq!(snapshot.count(slot(11)), 64);
    assert_eq!(snapshot.get(slot(slots::CARRIED)), None);
}

#[test]
fn a_middle_drag_outside_creative_moves_nothing() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(10)));
    let ops = drag(&mut snapshot, QuickCraftKind::Full, &[9, 10], false);
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 10);
    assert_eq!(snapshot.get(slot(9)), None);
    assert_eq!(snapshot.get(slot(10)), None);
}

#[test]
fn pickup_all_takes_partial_stacks_before_full_ones_going_forwards() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(5)));
    snapshot.set(slot(9), Some(stone(64)));
    snapshot.set(slot(10), Some(stone(10)));
    snapshot.set(slot(36), Some(stone(20)));
    let ops = click(&mut snapshot, ContainerInput::PickupAll, 12, 0);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(10),
                to: slot(slots::CARRIED),
                count: 10
            },
            Op::Transfer {
                from: slot(36),
                to: slot(slots::CARRIED),
                count: 20
            },
            Op::Transfer {
                from: slot(9),
                to: slot(slots::CARRIED),
                count: 29
            },
        ]
    );
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 64);
    assert_eq!(snapshot.count(slot(9)), 35);
    assert_eq!(snapshot.get(slot(10)), None);
    assert_eq!(snapshot.get(slot(36)), None);
}

#[test]
fn pickup_all_goes_backwards_for_button_1() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(5)));
    snapshot.set(slot(9), Some(stone(10)));
    snapshot.set(slot(36), Some(stone(20)));
    let ops = click(&mut snapshot, ContainerInput::PickupAll, 12, 1);
    assert_eq!(
        ops,
        [
            Op::Transfer {
                from: slot(36),
                to: slot(slots::CARRIED),
                count: 20
            },
            Op::Transfer {
                from: slot(9),
                to: slot(slots::CARRIED),
                count: 10
            },
        ]
    );
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 35);
}

#[test]
fn pickup_all_stops_when_the_cursor_is_full() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(60)));
    snapshot.set(slot(9), Some(stone(10)));
    snapshot.set(slot(10), Some(stone(10)));
    let ops = click(&mut snapshot, ContainerInput::PickupAll, 12, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(9),
            to: slot(slots::CARRIED),
            count: 4
        }]
    );
    assert_eq!(snapshot.count(slot(slots::CARRIED)), 64);
    assert_eq!(snapshot.count(slot(10)), 10);
}

#[test]
fn pickup_all_needs_a_carried_stack_and_an_empty_or_unpickable_clicked_slot() {
    let mut snapshot = fresh();
    snapshot.set(slot(9), Some(stone(10)));
    let ops = click(&mut snapshot, ContainerInput::PickupAll, 12, 0);
    assert!(ops.is_empty(), "{ops:?}");

    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(5)));
    snapshot.set(slot(9), Some(stone(10)));
    let ops = click(&mut snapshot, ContainerInput::PickupAll, 9, 0);
    assert!(ops.is_empty(), "{ops:?}");
}

#[test]
fn pickup_all_leaves_other_items_alone() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(stone(5)));
    snapshot.set(slot(9), Some(stone(10)));
    snapshot.set(slot(36), Some(helmet()));
    let ops = click(&mut snapshot, ContainerInput::PickupAll, 12, 0);
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(9),
            to: slot(slots::CARRIED),
            count: 10
        }]
    );
    assert_eq!(snapshot.get(slot(36)), Some(&helmet()));
}

#[test]
fn a_pickup_fills_the_held_slot_first_then_a_free_slot() {
    let mut snapshot = fresh();
    for index in slots::HOTBAR.chain(slots::MAIN) {
        let count = if index == slots::held(3) { 60 } else { 64 };
        snapshot.set(slot(index), Some(stone(count)));
    }
    snapshot.set(slot(slots::MAIN.start), None);
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
                to: slot(slots::held(3)),
                count: 4
            },
            Op::Pickup {
                item,
                to: slot(slots::MAIN.start),
                count: 3
            },
        ]
    );
}

#[test]
fn leggings_go_into_the_leggings_slot_and_not_the_head_slot() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::CARRIED), Some(leggings()));
    let ops = click(
        &mut snapshot,
        ContainerInput::Pickup,
        slots::ARMOR_HEAD as i16,
        0,
    );
    assert_eq!(ops, []);

    let ops = click(
        &mut snapshot,
        ContainerInput::Pickup,
        slots::ARMOR_LEGS as i16,
        0,
    );
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(slots::CARRIED),
            to: slot(slots::ARMOR_LEGS),
            count: 1
        }]
    );
}

#[test]
fn a_sword_is_refused_by_every_armour_slot() {
    for index in slots::ARMOR_HEAD..=slots::ARMOR_FEET {
        let mut snapshot = fresh();
        snapshot.set(slot(slots::CARRIED), Some(sword()));
        let ops = click(&mut snapshot, ContainerInput::Pickup, index as i16, 0);
        assert_eq!(ops, [], "slot {index}");
        assert_eq!(snapshot.get(slot(index)), None);
    }
}

#[test]
fn swapping_onto_worn_armour_is_gated_by_the_incoming_stack() {
    let other_helmet = StackView {
        key: StackKey {
            item: ItemId(5),
            components: ComponentPatch::EMPTY,
        },
        ..helmet()
    };
    let mut snapshot = fresh();
    snapshot.set(slot(slots::ARMOR_HEAD), Some(helmet()));
    snapshot.set(slot(slots::CARRIED), Some(other_helmet.clone()));
    let ops = click(
        &mut snapshot,
        ContainerInput::Pickup,
        slots::ARMOR_HEAD as i16,
        0,
    );
    assert_eq!(
        ops,
        [Op::Swap {
            a: slot(slots::ARMOR_HEAD),
            b: slot(slots::CARRIED)
        }]
    );
    assert_eq!(snapshot.get(slot(slots::ARMOR_HEAD)), Some(&other_helmet));

    let mut snapshot = fresh();
    snapshot.set(slot(slots::ARMOR_HEAD), Some(helmet()));
    snapshot.set(slot(slots::CARRIED), Some(sword()));
    let ops = click(
        &mut snapshot,
        ContainerInput::Pickup,
        slots::ARMOR_HEAD as i16,
        0,
    );
    assert_eq!(ops, []);
    assert_eq!(snapshot.get(slot(slots::ARMOR_HEAD)), Some(&helmet()));
    assert_eq!(snapshot.get(slot(slots::CARRIED)), Some(&sword()));
}

#[test]
fn armour_the_player_may_not_wear_is_refused_by_its_own_slot() {
    let mut snapshot = fresh();
    snapshot.set(
        slot(slots::CARRIED),
        Some(StackView {
            armour: None,
            ..helmet()
        }),
    );
    let ops = click(
        &mut snapshot,
        ContainerInput::Pickup,
        slots::ARMOR_HEAD as i16,
        0,
    );
    assert_eq!(ops, []);
}

const CHEST: i16 = slots::ARMOR_CHEST as i16;

/// The ops of one click on a snapshot wearing the cursed chestplate, in
/// survival and then in creative.
fn against_the_curse(
    setup: fn(&mut MenuSnapshot),
    input: ContainerInput,
    index: i16,
    button: u8,
) -> [Vec<Op>; 2] {
    [false, true].map(|creative| {
        let mut snapshot = fresh();
        snapshot.set(slot(slots::ARMOR_CHEST), Some(cursed_chestplate()));
        setup(&mut snapshot);
        click_as(&mut snapshot, input, index, button, creative)
    })
}

#[test]
fn a_left_click_takes_cursed_armour_only_in_creative() {
    let [survival, creative] = against_the_curse(|_| {}, ContainerInput::Pickup, CHEST, 0);
    assert_eq!(survival, []);
    assert_eq!(
        creative,
        [Op::Transfer {
            from: slot(slots::ARMOR_CHEST),
            to: slot(slots::CARRIED),
            count: 1
        }]
    );
}

#[test]
fn a_right_click_takes_cursed_armour_only_in_creative() {
    let [survival, creative] = against_the_curse(|_| {}, ContainerInput::Pickup, CHEST, 1);
    assert_eq!(survival, []);
    assert_eq!(
        creative,
        [Op::Transfer {
            from: slot(slots::ARMOR_CHEST),
            to: slot(slots::CARRIED),
            count: 1
        }]
    );
}

#[test]
fn armour_on_the_cursor_swaps_with_cursed_armour_only_in_creative() {
    let [survival, creative] = against_the_curse(
        |snapshot| snapshot.set(slot(slots::CARRIED), Some(chestplate())),
        ContainerInput::Pickup,
        CHEST,
        0,
    );
    assert_eq!(survival, []);
    assert_eq!(
        creative,
        [Op::Swap {
            a: slot(slots::ARMOR_CHEST),
            b: slot(slots::CARRIED)
        }]
    );
}

#[test]
fn a_digit_key_swaps_cursed_armour_with_a_hotbar_stack_only_in_creative() {
    let [survival, creative] = against_the_curse(
        |snapshot| snapshot.set(slot(slots::held(0)), Some(chestplate())),
        ContainerInput::Swap,
        CHEST,
        0,
    );
    assert_eq!(survival, []);
    assert_eq!(
        creative,
        [Op::Swap {
            a: slot(slots::ARMOR_CHEST),
            b: slot(slots::held(0))
        }]
    );
}

#[test]
fn a_digit_key_takes_cursed_armour_into_an_empty_hotbar_slot_only_in_creative() {
    let [survival, creative] = against_the_curse(|_| {}, ContainerInput::Swap, CHEST, 0);
    assert_eq!(survival, []);
    assert_eq!(
        creative,
        [Op::Transfer {
            from: slot(slots::ARMOR_CHEST),
            to: slot(slots::held(0)),
            count: 1
        }]
    );
}

#[test]
fn throwing_cursed_armour_drops_it_only_in_creative() {
    for button in [0, 1] {
        let [survival, creative] = against_the_curse(|_| {}, ContainerInput::Throw, CHEST, button);
        assert_eq!(survival, [], "button {button}");
        assert_eq!(
            creative,
            [Op::Drop {
                from: slot(slots::ARMOR_CHEST),
                count: 1,
                thrower: player()
            }],
            "button {button}"
        );
    }
}

#[test]
fn a_shift_click_moves_cursed_armour_only_in_creative() {
    let [survival, creative] = against_the_curse(|_| {}, ContainerInput::QuickMove, CHEST, 0);
    assert_eq!(survival, []);
    assert_eq!(
        creative,
        [Op::Transfer {
            from: slot(slots::ARMOR_CHEST),
            to: slot(slots::MAIN.start),
            count: 1
        }]
    );
}

#[test]
fn a_double_click_gathers_cursed_armour_only_in_creative() {
    let [survival, creative] = against_the_curse(
        |snapshot| {
            let stackable = StackView {
                max: 64,
                stackable: true,
                ..cursed_chestplate()
            };
            snapshot.set(slot(slots::ARMOR_CHEST), Some(stackable.clone()));
            snapshot.set(slot(slots::CARRIED), Some(stackable));
        },
        ContainerInput::PickupAll,
        slots::MAIN.start as i16,
        0,
    );
    assert_eq!(survival, []);
    assert_eq!(
        creative,
        [Op::Transfer {
            from: slot(slots::ARMOR_CHEST),
            to: slot(slots::CARRIED),
            count: 1
        }]
    );
}

#[test]
fn cursed_armour_outside_the_armour_slots_is_taken_in_survival() {
    let mut snapshot = fresh();
    snapshot.set(slot(slots::MAIN.start), Some(cursed_chestplate()));
    let ops = click(
        &mut snapshot,
        ContainerInput::Pickup,
        slots::MAIN.start as i16,
        0,
    );
    assert_eq!(
        ops,
        [Op::Transfer {
            from: slot(slots::MAIN.start),
            to: slot(slots::CARRIED),
            count: 1
        }]
    );
}
