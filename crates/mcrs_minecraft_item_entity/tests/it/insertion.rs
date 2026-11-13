use std::collections::BTreeMap;

use mcrs_minecraft_item_entity::Insertion;
use mcrs_minecraft_item_entity::OFF_HAND;
use mcrs_minecraft_item_entity::Slot;
use mcrs_minecraft_item_entity::insert;

/// Inserts `count` alike items of maximum 64 into slots that hold the counts of `alike`; each
/// slot of `others` holds a stack that is not alike.
fn run(
    count: u8,
    damaged: bool,
    selected: u8,
    alike: &[(u16, u8)],
    others: &[u16],
) -> (Vec<(u16, u8)>, Insertion) {
    let alike: BTreeMap<u16, u8> = alike.iter().copied().collect();
    let slot = |index: u16| match alike.get(&index) {
        Some(held) => Slot::Alike { room: 64 - held },
        None if others.contains(&index) => Slot::Other,
        None => Slot::Empty,
    };
    let mut merges = Vec::new();
    let end = insert(count, 64, damaged, selected, slot, |index, moved| {
        merges.push((index, moved));
    });
    (merges, end)
}

#[test]
fn ien_pck_003_the_selected_slot_then_the_off_hand_then_the_main_slots() {
    let full = [(0, 60), (2, 60), (OFF_HAND, 60)];
    let (merges, _) = run(4, false, 2, &full, &[]);
    assert_eq!(merges, [(2, 4)], "R16: the selected slot 2 first");
    let (merges, _) = run(4, false, 2, &[(0, 60), (OFF_HAND, 60)], &[]);
    assert_eq!(
        merges,
        [(OFF_HAND, 4)],
        "R16: the off-hand before the hotbar slot 0"
    );
    let (merges, end) = run(20, false, 2, &full, &[]);
    assert_eq!(merges, [(2, 4), (OFF_HAND, 4), (0, 4)]);
    assert_eq!(
        end,
        Insertion {
            empty: Some(1),
            filled: 8,
            rest: 0
        }
    );
}

#[test]
fn ien_pck_003_a_damaged_stack_goes_only_to_the_first_empty_main_slot() {
    let (merges, end) = run(1, true, 0, &[(0, 1)], &[1]);
    assert!(merges.is_empty(), "no merge: {merges:?}");
    assert_eq!(end.empty, Some(2));
}

#[test]
fn ien_pck_005_the_items_that_find_no_room_stay() {
    let others: Vec<u16> = (0..35).collect();
    let (merges, end) = run(10, false, 0, &[(35, 60)], &others);
    assert_eq!(merges, [(35, 4)]);
    assert_eq!(
        end,
        Insertion {
            empty: None,
            filled: 0,
            rest: 6
        },
        "R17: 6 stay in the item entity"
    );
}
