use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;
use mcrs_minecraft_inventory_click::Drag;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_menu::MenuSlot;

use crate::fixture::CURSOR;
use crate::fixture::Operation;
use crate::fixture::Player;
use crate::fixture::Stack;
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

/// The operations of the end packet of a drag of `kind` over `slots`. Panics when another packet
/// gives operations or when the drag is not idle after the end packet.
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

fn set(slot: MenuSlot, stack: Stack) -> Operation {
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
