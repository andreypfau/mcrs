use mcrs_minecraft_inventory_plan::Entry;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Plan;
use mcrs_minecraft_inventory_plan::Planned;

use crate::holder::Slot;
use crate::holder::Stack;
use crate::holder::dirt;
use crate::holder::drop_from;
use crate::holder::limited;
use crate::holder::locked;
use crate::holder::merge;
use crate::holder::more_stone;
use crate::holder::open;
use crate::holder::pearl;
use crate::holder::request;
use crate::holder::request_in;
use crate::holder::split;
use crate::holder::stone;
use crate::holder::swap;

struct Case {
    name: &'static str,
    slots: Vec<Slot>,
    ops: Vec<Op<usize>>,
    after: Vec<Stack>,
    dropped: Vec<Stack>,
}

fn oversized() -> Stack {
    Some(Planned {
        stack: 10,
        kind: 1,
        count: 80,
        max: 64,
    })
}

#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "an empty request changes nothing",
            slots: vec![open(stone(10))],
            ops: vec![],
            after: vec![stone(10)],
            dropped: vec![],
        },
        Case {
            name: "split makes a new stack in an empty slot",
            slots: vec![open(stone(10)), open(None)],
            ops: vec![split(0, 1, 4)],
            after: vec![stone(6), stone(4)],
            dropped: vec![],
        },
        Case {
            name: "split of the whole count empties the source and grows the destination stack",
            slots: vec![open(stone(10)), open(more_stone(5))],
            ops: vec![split(0, 1, 10)],
            after: vec![None, more_stone(15)],
            dropped: vec![],
        },
        Case {
            name: "split fills a slot up to its slot limit",
            slots: vec![open(stone(10)), limited(None, 4)],
            ops: vec![split(0, 1, 4)],
            after: vec![stone(6), stone(4)],
            dropped: vec![],
        },
        Case {
            name: "split fills a stack up to its maximum stack size",
            slots: vec![open(pearl(10)), open(pearl(10))],
            ops: vec![split(0, 1, 6)],
            after: vec![pearl(4), pearl(16)],
            dropped: vec![],
        },
        Case {
            name: "split enters a slot whose take filter refuses",
            slots: vec![open(stone(10)), locked(None)],
            ops: vec![split(0, 1, 3)],
            after: vec![stone(7), stone(3)],
            dropped: vec![],
        },
        Case {
            name: "merge moves the whole stack when it fits",
            slots: vec![open(stone(10)), open(more_stone(20))],
            ops: vec![merge(0, 1)],
            after: vec![None, more_stone(30)],
            dropped: vec![],
        },
        Case {
            name: "merge leaves the rest when the maximum stack size stops it",
            slots: vec![open(pearl(10)), open(pearl(10))],
            ops: vec![merge(0, 1)],
            after: vec![pearl(4), pearl(16)],
            dropped: vec![],
        },
        Case {
            name: "merge leaves the rest when the slot limit stops it",
            slots: vec![open(stone(10)), limited(None, 5)],
            ops: vec![merge(0, 1)],
            after: vec![stone(5), stone(5)],
            dropped: vec![],
        },
        Case {
            name: "swap exchanges two different stacks",
            slots: vec![open(stone(3)), open(dirt(7))],
            ops: vec![swap(0, 1)],
            after: vec![dirt(7), stone(3)],
            dropped: vec![],
        },
        Case {
            name: "swap moves one stack to an empty slot",
            slots: vec![open(None), open(dirt(7))],
            ops: vec![swap(0, 1)],
            after: vec![dirt(7), None],
            dropped: vec![],
        },
        Case {
            name: "swap does not join equal stacks",
            slots: vec![open(stone(3)), open(more_stone(5))],
            ops: vec![swap(0, 1)],
            after: vec![more_stone(5), stone(3)],
            dropped: vec![],
        },
        Case {
            name: "swap moves a stack over its maximum stack size as it is",
            slots: vec![open(oversized()), open(None)],
            ops: vec![swap(0, 1)],
            after: vec![None, oversized()],
            dropped: vec![],
        },
        Case {
            name: "drop removes a part of a stack",
            slots: vec![open(stone(10))],
            ops: vec![drop_from(0, 3)],
            after: vec![stone(7)],
            dropped: vec![stone(3)],
        },
        Case {
            name: "drop of the whole count empties the slot",
            slots: vec![open(stone(10))],
            ops: vec![drop_from(0, 10)],
            after: vec![None],
            dropped: vec![stone(10)],
        },
        Case {
            name: "each drop gives its own dropped stack in the order of the operations",
            slots: vec![open(stone(10)), open(dirt(5))],
            ops: vec![drop_from(1, 2), drop_from(0, 1), drop_from(1, 3)],
            after: vec![stone(9), None],
            dropped: vec![dirt(2), stone(1), dirt(3)],
        },
        Case {
            name: "a later operation uses the stack that an earlier operation moved",
            slots: vec![open(stone(10)), open(None), open(None)],
            ops: vec![swap(0, 1), split(1, 2, 4), merge(2, 0), drop_from(0, 1)],
            after: vec![stone(3), stone(6), None],
            dropped: vec![stone(1)],
        },
    ]
}

#[test]
fn an_applied_request_gives_the_stacks_and_the_dropped_stacks_of_its_operations() {
    for case in cases() {
        let outcome = request(&case.slots, &case.ops);
        assert_eq!(outcome.result, Ok(()), "{}", case.name);
        assert_eq!(outcome.after, case.after, "{}", case.name);
        let dropped: Vec<Stack> = outcome.dropped.into_iter().map(Some).collect();
        assert_eq!(dropped, case.dropped, "{}", case.name);
    }
}

#[test]
fn an_applied_request_reports_only_the_slots_whose_stack_changed() {
    let slots = [open(stone(10)), open(None), open(dirt(1)), open(None)];
    let ops = [split(0, 1, 4), swap(2, 3), swap(3, 2)];
    let mut scratch = Plan::new();
    let outcome = request_in(&mut scratch, &slots, ops);
    assert_eq!(outcome.result, Ok(()));
    let changes: Vec<Entry<usize, u16>> = scratch.changes().collect();
    assert_eq!(
        changes,
        [
            Entry {
                slot: 0,
                before: stone(10),
                after: stone(6)
            },
            Entry {
                slot: 1,
                before: None,
                after: stone(4)
            },
        ]
    );
}
