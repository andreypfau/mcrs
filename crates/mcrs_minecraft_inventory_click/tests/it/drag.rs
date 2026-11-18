use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_click::Drag;
use mcrs_minecraft_inventory_click::DragKind;
use mcrs_minecraft_inventory_click::DragSplit;
use mcrs_minecraft_inventory_click::EntryPreview;
use mcrs_minecraft_inventory_transaction::Op;

use crate::fixture::CURSOR;
use crate::fixture::HeldSlot;
use crate::fixture::Operation;
use crate::fixture::Player;
use crate::fixture::Stack;
use crate::fixture::Window;
use crate::fixture::apply;
use crate::fixture::dirt;
use crate::fixture::entry;
use crate::fixture::pumpkin;
use crate::fixture::send;
use crate::fixture::split;
use crate::fixture::stone;

const SPLIT: u8 = 0;
const SINGLE: u8 = 1;
const FULL: u8 = 2;

fn button(kind: u8, stage: u8) -> u8 {
    kind << 2 | stage
}

fn quick_craft(player: &Player, state: &mut Drag, button: u8, clicked: Clicked) -> Vec<Operation> {
    send(player, state, ClickKind::Drag, button, clicked).expect("each drag packet names an entry")
}

fn drag(player: &Player, kind: u8, slots: &[Clicked]) -> Vec<Operation> {
    let mut state = Drag::new();
    let mut before_end = quick_craft(player, &mut state, button(kind, 0), Clicked::Outside);
    for clicked in slots {
        before_end.extend(quick_craft(player, &mut state, button(kind, 1), *clicked));
    }
    assert_eq!(before_end, vec![], "only the end packet gives operations");
    let ops = quick_craft(player, &mut state, button(kind, 2), Clicked::Outside);
    assert_eq!(state.kind(), None, "the end packet resets the drag");
    ops
}

fn set(slot: HeldSlot, stack: Stack) -> Operation {
    Op::Set {
        slot,
        stack,
        count: stack.map_or(0, |stack| stack.count),
    }
}

struct Row {
    name: &'static str,
    player: Player,
    kind: u8,
    slots: Vec<Clicked>,
    ops: Vec<Operation>,
    after: Player,
}

fn check(rows: Vec<Row>) {
    for row in rows {
        let ops = drag(&row.player, row.kind, &row.slots);
        assert_eq!(ops, row.ops, "{}", row.name);
        assert_eq!(apply(&row.player, &ops), row.after, "{}", row.name);
    }
}

fn entries(indices: &[u16]) -> Vec<Clicked> {
    indices.iter().copied().map(Clicked::Entry).collect()
}

#[test]
fn itx_jav_017_018_019_end_packet_splits_the_cursor_over_the_set() {
    let near_full = || Player::new().carrying(stone(10)).with(10, stone(62));
    check(vec![
        Row {
            name: "drag kind 0 gives an equal part, in the order of the menu index",
            player: Player::new().carrying(stone(10)),
            kind: SPLIT,
            slots: entries(&[11, 9, 10]),
            ops: vec![
                split(CURSOR, entry(9), 3),
                split(CURSOR, entry(10), 3),
                split(CURSOR, entry(11), 3),
            ],
            after: Player::new()
                .carrying(stone(1))
                .with(9, stone(3))
                .with(10, stone(3))
                .with(11, stone(3)),
        },
        Row {
            name: "drag kind 0 stops at the capacity of a slot",
            player: near_full(),
            kind: SPLIT,
            slots: entries(&[9, 10, 11]),
            ops: vec![
                split(CURSOR, entry(9), 3),
                split(CURSOR, entry(10), 2),
                split(CURSOR, entry(11), 3),
            ],
            after: Player::new()
                .carrying(stone(2))
                .with(9, stone(3))
                .with(10, stone(64))
                .with(11, stone(3)),
        },
        Row {
            name: "drag kind 1 gives one item",
            player: near_full(),
            kind: SINGLE,
            slots: entries(&[9, 10, 11]),
            ops: vec![
                split(CURSOR, entry(9), 1),
                split(CURSOR, entry(10), 1),
                split(CURSOR, entry(11), 1),
            ],
            after: Player::new()
                .carrying(stone(7))
                .with(9, stone(1))
                .with(10, stone(63))
                .with(11, stone(1)),
        },
        Row {
            name: "drag kind 1 adds no slot past the count of the cursor",
            player: Player::new().carrying(stone(2)),
            kind: SINGLE,
            slots: entries(&[9, 10, 11]),
            ops: vec![split(CURSOR, entry(9), 1), split(CURSOR, entry(10), 1)],
            after: Player::new().with(9, stone(1)).with(10, stone(1)),
        },
        Row {
            name: "a refused place, another item, a repeat and a negative index add no slot",
            player: Player::new().carrying(stone(9)).with(11, dirt(1)),
            kind: SPLIT,
            slots: vec![
                Clicked::Entry(5),
                Clicked::Entry(9),
                Clicked::Entry(9),
                Clicked::Entry(11),
                Clicked::Nowhere,
                Clicked::Entry(10),
            ],
            ops: vec![split(CURSOR, entry(9), 4), split(CURSOR, entry(10), 4)],
            after: Player::new()
                .carrying(stone(1))
                .with(9, stone(4))
                .with(10, stone(4))
                .with(11, dirt(1)),
        },
        Row {
            name: "an armour slot takes up to its slot limit",
            player: Player::new().carrying(pumpkin(0, 10)),
            kind: SPLIT,
            slots: entries(&[9, 5]),
            ops: vec![split(CURSOR, entry(5), 1), split(CURSOR, entry(9), 5)],
            after: Player::new()
                .carrying(pumpkin(0, 4))
                .with(5, pumpkin(0, 1))
                .with(9, pumpkin(0, 5)),
        },
    ]);
}

#[test]
fn itx_jav_018_a_drag_over_one_slot_is_a_pick_up() {
    check(vec![
        Row {
            name: "drag kind 0 over one slot is the pick up click with button 0",
            player: Player::new().carrying(stone(10)).with(10, stone(62)),
            kind: SPLIT,
            slots: entries(&[10]),
            ops: vec![split(CURSOR, entry(10), 2)],
            after: Player::new().carrying(stone(8)).with(10, stone(64)),
        },
        Row {
            name: "drag kind 1 over one slot is the pick up click with button 1",
            player: Player::new().carrying(stone(10)),
            kind: SINGLE,
            slots: entries(&[9]),
            ops: vec![split(CURSOR, entry(9), 1)],
            after: Player::new().carrying(stone(9)).with(9, stone(1)),
        },
        Row {
            name: "no slot",
            player: Player::new().carrying(stone(10)),
            kind: SPLIT,
            slots: vec![],
            ops: vec![],
            after: Player::new().carrying(stone(10)),
        },
    ]);
}

#[test]
fn itx_jav_020_inv_cre_005_full_drag_sets_each_slot_to_a_full_stack() {
    let creative = || Player::new().creative();
    check(vec![
        Row {
            name: "the cursor runs out",
            player: creative().carrying(stone(10)).with(10, stone(60)),
            kind: FULL,
            slots: entries(&[5, 9, 10]),
            ops: vec![
                set(entry(9), stone(64)),
                set(entry(10), stone(64)),
                set(CURSOR, stone(0)),
            ],
            after: creative().with(9, stone(64)).with(10, stone(64)),
        },
        Row {
            name: "the cursor keeps the rest, and a full slot does not change",
            player: creative()
                .carrying(stone(64))
                .with(9, stone(60))
                .with(10, stone(64)),
            kind: FULL,
            slots: entries(&[9, 10]),
            ops: vec![set(entry(9), stone(64)), set(CURSOR, stone(60))],
            after: creative()
                .carrying(stone(60))
                .with(9, stone(64))
                .with(10, stone(64)),
        },
        Row {
            name: "one slot gives nothing",
            player: creative().carrying(stone(10)),
            kind: FULL,
            slots: entries(&[9]),
            ops: vec![],
            after: creative().carrying(stone(10)),
        },
    ]);
}

#[test]
fn itx_jav_016_a_packet_that_does_not_fit_resets_the_drag() {
    let packet = |button| (ClickKind::Drag, button, Clicked::Outside);
    let slot = |index| (ClickKind::Drag, button(SPLIT, 1), Clicked::Entry(index));
    let start = packet(button(SPLIT, 0));
    let end = packet(button(SPLIT, 2));
    let cases = [
        ("no start packet", vec![slot(9), slot(10), end]),
        (
            "two start packets",
            vec![start, start, slot(9), slot(10), end],
        ),
        (
            "the stage 3",
            vec![start, slot(9), packet(3), slot(10), end],
        ),
        (
            "a pick up click while the drag is open",
            vec![
                start,
                slot(9),
                (ClickKind::PickUp, 0, Clicked::Entry(30)),
                slot(10),
                end,
            ],
        ),
        (
            "the drag kind 2 for a player without infinite materials",
            vec![
                packet(button(FULL, 0)),
                (ClickKind::Drag, button(FULL, 1), Clicked::Entry(9)),
                (ClickKind::Drag, button(FULL, 1), Clicked::Entry(10)),
                packet(button(FULL, 2)),
            ],
        ),
        (
            "the drag kind 3",
            vec![packet(12), slot(9), slot(10), packet(14)],
        ),
    ];
    let player = Player::new().carrying(stone(5));
    for (name, packets) in cases {
        let mut state = Drag::new();
        for (kind, button, clicked) in packets {
            let ops = send(&player, &mut state, kind, button, clicked);
            assert_eq!(ops, Ok(vec![]), "{name}");
        }
        assert_eq!(state.kind(), None, "{name}");
    }
    let mut state = Drag::new();
    quick_craft(
        &Player::new(),
        &mut state,
        button(SPLIT, 0),
        Clicked::Outside,
    );
    assert_eq!(state.kind(), None, "an empty cursor does not open a drag");

    let creative = Player::new().creative().carrying(stone(5));
    let mut state = Drag::new();
    quick_craft(&creative, &mut state, button(FULL, 0), Clicked::Outside);
    quick_craft(&creative, &mut state, button(FULL, 1), Clicked::Entry(9));
    quick_craft(&creative, &mut state, button(FULL, 1), Clicked::Entry(10));
    let ops = quick_craft(&player, &mut state, button(FULL, 2), Clicked::Outside);
    assert_eq!(
        ops,
        vec![],
        "a player who left creative mode before the end makes no items"
    );
}

struct PreviewRow {
    name: &'static str,
    player: Player,
    kind: u8,
    slots: &'static [u16],
    previews: &'static [u8],
    cursor: u8,
    capped: Option<u16>,
}

fn count(stack: Stack) -> u8 {
    stack.map_or(0, |stack| stack.count)
}

#[test]
fn ccs_drg_005_006_preview_counts_equal_the_end_of_the_drag() {
    let rows = [
        PreviewRow {
            name: "drag kind 0 of 10 stone over three empty entries",
            player: Player::new().carrying(stone(10)),
            kind: SPLIT,
            slots: &[9, 10, 11],
            previews: &[3, 3, 3],
            cursor: 1,
            capped: None,
        },
        PreviewRow {
            name: "drag kind 0 of 4 stone over two empty entries leaves nothing at the mouse",
            player: Player::new().carrying(stone(4)),
            kind: SPLIT,
            slots: &[9, 10],
            previews: &[2, 2],
            cursor: 0,
            capped: None,
        },
        PreviewRow {
            name: "drag kind 1 that fills an entry to the maximum is not capped",
            player: Player::new().carrying(stone(64)).with(9, stone(63)),
            kind: SINGLE,
            slots: &[9, 10],
            previews: &[64, 1],
            cursor: 62,
            capped: None,
        },
        PreviewRow {
            name: "drag kind 0 caps an entry at its capacity",
            player: Player::new().carrying(stone(64)).with(9, stone(60)),
            kind: SPLIT,
            slots: &[9, 10],
            previews: &[64, 32],
            cursor: 28,
            capped: Some(9),
        },
        PreviewRow {
            name: "drag kind 2 in creative mode fills each entry and shows the maximum at the mouse",
            player: Player::new()
                .creative()
                .carrying(stone(5))
                .with(9, stone(10)),
            kind: FULL,
            slots: &[9, 10],
            previews: &[64, 64],
            cursor: 64,
            capped: Some(9),
        },
    ];
    for row in rows {
        let carried = row.player.cursor_stack().expect("each row carries a stack");
        let kind = DragKind::from_button(row.kind << 2).expect("each row names a drag kind");
        let split = DragSplit {
            kind,
            count: carried.count,
            max: carried.max,
            size: u32::try_from(row.slots.len()).expect("a row has few entries"),
        };
        let drag_entry = |index: u16| {
            mcrs_minecraft_inventory_click::joins_drag(&Window(&row.player), &row.player, index)
                .expect("each entry joins the drag")
        };
        let after = apply(
            &row.player,
            &drag(&row.player, row.kind, &entries(row.slots)),
        );
        for (&index, &expected) in row.slots.iter().zip(row.previews) {
            let preview = split
                .entry(drag_entry(index))
                .expect("a drag of two or more entries shows a preview");
            assert_eq!(preview.count(), expected, "{}: entry {index}", row.name);
            assert_eq!(
                count(after.at(index)),
                expected,
                "{}: entry {index} after the release",
                row.name
            );
            assert_eq!(
                matches!(preview, EntryPreview::Capped(_)),
                row.capped == Some(index),
                "{}: the capped text of entry {index}",
                row.name
            );
        }
        let cursor = split.cursor(row.slots.iter().copied().map(drag_entry));
        assert_eq!(cursor, row.cursor, "{}: the stack at the mouse", row.name);
        if kind != DragKind::Full {
            assert_eq!(
                count(after.cursor_stack()),
                row.cursor,
                "{}: the cursor after the release",
                row.name
            );
        }
    }
}

struct MembershipRow {
    name: &'static str,
    player: Player,
    joins: bool,
    stays: bool,
}

#[test]
fn ccs_drg_002_005_an_entry_that_refuses_the_item_stays_in_the_drag_but_does_not_join_it() {
    let rows = [
        MembershipRow {
            name: "an empty entry",
            player: Player::new().carrying(stone(10)),
            joins: true,
            stays: true,
        },
        MembershipRow {
            name: "an entry that refuses the cursor item",
            player: Player::new().carrying(stone(10)).refusing(9),
            joins: false,
            stays: true,
        },
        MembershipRow {
            name: "an entry with another item",
            player: Player::new().carrying(stone(10)).with(9, dirt(3)),
            joins: false,
            stays: false,
        },
        MembershipRow {
            name: "an empty cursor",
            player: Player::new(),
            joins: false,
            stays: false,
        },
    ];
    for row in rows {
        let window = Window(&row.player);
        assert_eq!(
            mcrs_minecraft_inventory_click::joins_drag(&window, &row.player, 9).is_some(),
            row.joins,
            "{}: joins",
            row.name
        );
        assert_eq!(
            mcrs_minecraft_inventory_click::stays_in_drag(&window, &row.player, 9).is_some(),
            row.stays,
            "{}: stays",
            row.name
        );
    }
}
