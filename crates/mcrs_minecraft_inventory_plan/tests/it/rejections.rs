use mcrs_minecraft_inventory_plan::MAX_OPS;
use mcrs_minecraft_inventory_plan::Plan;
use mcrs_minecraft_inventory_plan::Reason;
use mcrs_minecraft_inventory_plan::Reason::*;
use mcrs_minecraft_inventory_plan::Rejected;

use crate::holder::DIRT_HANDLE;
use crate::holder::Operation;
use crate::holder::STONE_HANDLE;
use crate::holder::Slot;
use crate::holder::dirt;
use crate::holder::drop_from;
use crate::holder::limited;
use crate::holder::locked;
use crate::holder::merge;
use crate::holder::named_stone;
use crate::holder::open;
use crate::holder::pearl;
use crate::holder::refusing;
use crate::holder::request;
use crate::holder::request_in;
use crate::holder::set;
use crate::holder::split;
use crate::holder::stone;
use crate::holder::swap;

struct Case {
    name: &'static str,
    slots: Vec<Slot>,
    ops: Vec<Operation>,
    op: usize,
    reason: Reason,
}

fn first(name: &'static str, slots: Vec<Slot>, op: Operation, reason: Reason) -> Case {
    Case {
        name,
        slots,
        ops: vec![op],
        op: 0,
        reason,
    }
}

fn stone_and_empty() -> Vec<Slot> {
    vec![open(stone(10)), open(None)]
}

#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn single_reason_cases() -> Vec<Case> {
    let no_stone = |slot| refusing(slot, STONE_HANDLE);
    let no_dirt = |slot| refusing(slot, DIRT_HANDLE);
    vec![
        first(
            "split from a missing slot",
            stone_and_empty(),
            split(9, 1, 1),
            NoSuchSlot,
        ),
        first(
            "split to a missing slot",
            stone_and_empty(),
            split(0, 9, 1),
            NoSuchSlot,
        ),
        first(
            // INV-CRE-001
            "set of a missing slot",
            stone_and_empty(),
            set(9, stone(1), 1),
            NoSuchSlot,
        ),
        first(
            "merge from a missing slot",
            stone_and_empty(),
            merge(9, 1),
            NoSuchSlot,
        ),
        first(
            "merge to a missing slot",
            stone_and_empty(),
            merge(0, 9),
            NoSuchSlot,
        ),
        first(
            "swap with a missing left slot",
            stone_and_empty(),
            swap(9, 0),
            NoSuchSlot,
        ),
        first(
            "swap with a missing right slot",
            stone_and_empty(),
            swap(0, 9),
            NoSuchSlot,
        ),
        first(
            "drop from a missing slot",
            stone_and_empty(),
            drop_from(9, 1),
            NoSuchSlot,
        ),
        first(
            "split to its own slot",
            stone_and_empty(),
            split(0, 0, 1),
            SameSlot,
        ),
        first(
            "merge to its own slot",
            stone_and_empty(),
            merge(0, 0),
            SameSlot,
        ),
        first(
            "swap with its own slot",
            stone_and_empty(),
            swap(0, 0),
            SameSlot,
        ),
        first(
            "split of no item",
            stone_and_empty(),
            split(0, 1, 0),
            BadCount,
        ),
        first(
            "drop of no item",
            stone_and_empty(),
            drop_from(0, 0),
            BadCount,
        ),
        first(
            "split from an empty slot",
            stone_and_empty(),
            split(1, 0, 1),
            EmptySource,
        ),
        first(
            "merge from an empty slot",
            stone_and_empty(),
            merge(1, 0),
            EmptySource,
        ),
        first(
            "drop from an empty slot",
            stone_and_empty(),
            drop_from(1, 1),
            EmptySource,
        ),
        first(
            "swap of two empty slots",
            vec![open(None), open(None)],
            swap(0, 1),
            EmptySource,
        ),
        first(
            "split of more than the slot holds",
            stone_and_empty(),
            split(0, 1, 11),
            NotEnough,
        ),
        first(
            "drop of more than the slot holds",
            stone_and_empty(),
            drop_from(0, 11),
            NotEnough,
        ),
        first(
            "split from a slot that refuses the take",
            vec![locked(stone(10)), open(None)],
            split(0, 1, 1),
            TakeDenied,
        ),
        first(
            "merge from a slot that refuses the take",
            vec![locked(stone(10)), open(None)],
            merge(0, 1),
            TakeDenied,
        ),
        first(
            "drop from a slot that refuses the take",
            vec![locked(stone(10))],
            drop_from(0, 1),
            TakeDenied,
        ),
        first(
            "swap whose left slot refuses the take",
            vec![locked(stone(10)), open(None)],
            swap(0, 1),
            TakeDenied,
        ),
        first(
            "swap whose right slot refuses the take",
            vec![open(stone(10)), locked(dirt(1))],
            swap(0, 1),
            TakeDenied,
        ),
        first(
            "split onto a different item",
            vec![open(stone(10)), open(dirt(1))],
            split(0, 1, 1),
            NotEqual,
        ),
        first(
            "split onto the same item with different components",
            vec![open(stone(10)), open(named_stone(1))],
            split(0, 1, 1),
            NotEqual,
        ),
        first(
            "merge onto a different item",
            vec![open(stone(10)), open(dirt(1))],
            merge(0, 1),
            NotEqual,
        ),
        first(
            "split into a slot that refuses the stack",
            vec![open(stone(10)), no_stone(open(None))],
            split(0, 1, 1),
            PlaceDenied,
        ),
        first(
            "merge into a slot that refuses the stack",
            vec![open(stone(10)), no_stone(open(stone(1)))],
            merge(0, 1),
            PlaceDenied,
        ),
        first(
            "swap whose right slot refuses the left stack",
            vec![open(stone(10)), no_stone(open(dirt(1)))],
            swap(0, 1),
            PlaceDenied,
        ),
        first(
            "swap whose left slot refuses the right stack",
            vec![no_dirt(open(stone(10))), open(dirt(1))],
            swap(0, 1),
            PlaceDenied,
        ),
        first(
            "split past the maximum stack size",
            vec![open(pearl(10)), open(pearl(10))],
            split(0, 1, 7),
            NoRoom,
        ),
        first(
            "split past the slot limit",
            vec![open(stone(10)), limited(None, 4)],
            split(0, 1, 5),
            NoRoom,
        ),
        first(
            "merge into a full stack",
            vec![open(pearl(10)), open(pearl(16))],
            merge(0, 1),
            NoRoom,
        ),
        first(
            "swap whose left stack is over the right slot limit",
            vec![open(stone(10)), limited(dirt(1), 9)],
            swap(0, 1),
            NoRoom,
        ),
        first(
            "swap whose right stack is over the left slot limit",
            vec![limited(None, 9), open(stone(10))],
            swap(0, 1),
            NoRoom,
        ),
    ]
}

#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn ordered_reason_cases() -> Vec<Case> {
    let no_stone = |slot| refusing(slot, STONE_HANDLE);
    vec![
        first(
            "a missing slot before the same slot",
            stone_and_empty(),
            split(9, 9, 0),
            NoSuchSlot,
        ),
        first(
            "the same slot before a bad count",
            stone_and_empty(),
            split(1, 1, 0),
            SameSlot,
        ),
        first(
            "a bad count before an empty source",
            stone_and_empty(),
            split(1, 0, 0),
            BadCount,
        ),
        first(
            "a bad count before an empty source of a drop",
            vec![locked(None)],
            drop_from(0, 0),
            BadCount,
        ),
        first(
            "an empty source before a refused take",
            vec![locked(None), open(dirt(1))],
            split(0, 1, 1),
            EmptySource,
        ),
        first(
            "an empty source of a merge before a refused take",
            vec![locked(None), open(dirt(1))],
            merge(0, 1),
            EmptySource,
        ),
        first(
            "a count that is too large before a refused take",
            vec![locked(stone(2)), open(dirt(1))],
            split(0, 1, 5),
            NotEnough,
        ),
        first(
            "a count that is too large before a refused take of a drop",
            vec![locked(stone(2))],
            drop_from(0, 5),
            NotEnough,
        ),
        first(
            "a refused take before stacks that are not equal",
            vec![locked(stone(2)), no_stone(open(dirt(64)))],
            split(0, 1, 1),
            TakeDenied,
        ),
        first(
            "a refused take of a merge before stacks that are not equal",
            vec![locked(stone(2)), no_stone(open(dirt(64)))],
            merge(0, 1),
            TakeDenied,
        ),
        first(
            "stacks that are not equal before a refused place",
            vec![open(stone(2)), no_stone(open(dirt(64)))],
            split(0, 1, 1),
            NotEqual,
        ),
        first(
            "stacks that are not equal before a refused place of a merge",
            vec![open(stone(2)), no_stone(open(dirt(64)))],
            merge(0, 1),
            NotEqual,
        ),
        first(
            "a refused place before no room",
            vec![open(stone(2)), no_stone(open(stone(64)))],
            split(0, 1, 1),
            PlaceDenied,
        ),
        first(
            "a refused place of a merge before no room",
            vec![open(stone(2)), no_stone(open(stone(64)))],
            merge(0, 1),
            PlaceDenied,
        ),
        first(
            "the same slot of a swap before two empty slots",
            vec![open(None)],
            swap(0, 0),
            SameSlot,
        ),
        first(
            "a refused take of a swap before a refused place",
            vec![locked(stone(10)), no_stone(limited(None, 1))],
            swap(0, 1),
            TakeDenied,
        ),
        first(
            "a refused place of a swap before no room",
            vec![open(stone(10)), no_stone(limited(None, 1))],
            swap(0, 1),
            PlaceDenied,
        ),
    ]
}

fn index_cases() -> Vec<Case> {
    vec![
        Case {
            name: "the rejection names the operation that fails",
            slots: stone_and_empty(),
            ops: vec![split(0, 1, 4), swap(0, 1), drop_from(1, 7)],
            op: 2,
            reason: NotEnough,
        },
        Case {
            name: "the first of two operations that fail decides",
            slots: stone_and_empty(),
            ops: vec![split(0, 1, 4), drop_from(0, 7), swap(9, 9)],
            op: 1,
            reason: NotEnough,
        },
        Case {
            name: "a check reads the working state, not the state at the start",
            slots: stone_and_empty(),
            ops: vec![swap(0, 1), drop_from(0, 1)],
            op: 1,
            reason: EmptySource,
        },
    ]
}

#[test]
fn a_rejection_has_the_first_reason_of_the_first_operation_that_fails() {
    let groups = [single_reason_cases(), ordered_reason_cases(), index_cases()];
    for case in groups.into_iter().flatten() {
        let outcome = request(&case.slots, &case.ops);
        let expected = Rejected {
            op: case.op,
            reason: case.reason,
        };
        assert_eq!(outcome.result, Err(expected), "{}", case.name);
    }
}

fn swaps(count: usize) -> impl Iterator<Item = Operation> {
    (0..count).map(|_| swap(0, 1))
}

#[test]
fn a_request_is_too_long_past_the_operation_maximum() {
    let slots = stone_and_empty();
    let mut scratch = Plan::new();

    let longest = request_in(&mut scratch, &slots, swaps(MAX_OPS));
    assert_eq!(longest.result, Ok(()));

    let too_many = Rejected {
        op: MAX_OPS,
        reason: TooManyOps,
    };
    let longer = request_in(&mut scratch, &slots, swaps(MAX_OPS + 1));
    assert_eq!(longer.result, Err(too_many));

    let bad_first = [drop_from(9, 0)].into_iter().chain(swaps(MAX_OPS));
    let longer_with_bad_operation = request_in(&mut scratch, &slots, bad_first);
    assert_eq!(longer_with_bad_operation.result, Err(too_many));
}
