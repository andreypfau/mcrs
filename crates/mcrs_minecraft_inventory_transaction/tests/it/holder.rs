use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_inventory_transaction::Loaded;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_inventory_transaction::Plan;
use mcrs_minecraft_inventory_transaction::Planned;
use mcrs_minecraft_inventory_transaction::Rejected;

pub type Stack = Option<Planned<u16>>;

pub type Operation = Op<usize, Planned<u16>>;

pub const STONE: u16 = 1;
pub const DIRT: u16 = 2;
pub const PEARL: u16 = 3;

pub fn components(handle: u16) -> u16 {
    handle / 10
}

fn stack(handle: u16, kind: u16, count: u8, max: u8) -> Stack {
    Some(Planned {
        stack: handle,
        kind,
        count,
        max,
    })
}

pub fn stone(count: u8) -> Stack {
    stack(10, STONE, count, 64)
}

pub fn more_stone(count: u8) -> Stack {
    stack(11, STONE, count, 64)
}

pub fn named_stone(count: u8) -> Stack {
    stack(40, STONE, count, 64)
}

pub fn dirt(count: u8) -> Stack {
    stack(20, DIRT, count, 64)
}

pub fn pearl(count: u8) -> Stack {
    stack(30, PEARL, count, 16)
}

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub stack: Stack,
    pub limit: u8,
    pub take: Filter,
    pub refuses: Option<u16>,
}

pub fn open(stack: Stack) -> Slot {
    Slot {
        stack,
        limit: 99,
        take: Filter::Permits,
        refuses: None,
    }
}

pub fn limited(stack: Stack, limit: u8) -> Slot {
    Slot {
        limit,
        ..open(stack)
    }
}

pub fn locked(stack: Stack) -> Slot {
    Slot {
        take: Filter::Refuses,
        ..open(stack)
    }
}

pub fn refusing(slot: Slot, handle: u16) -> Slot {
    Slot {
        refuses: Some(components(handle)),
        ..slot
    }
}

pub const STONE_HANDLE: u16 = 10;
pub const DIRT_HANDLE: u16 = 20;

pub fn split(from: usize, to: usize, count: u8) -> Operation {
    Op::Split { from, to, count }
}

pub fn merge(from: usize, to: usize) -> Operation {
    Op::Merge { from, to }
}

pub fn swap(left: usize, right: usize) -> Operation {
    Op::Swap { left, right }
}

pub fn drop_from(from: usize, count: u8) -> Operation {
    Op::Drop { from, count }
}

pub fn set(slot: usize, stack: Stack, count: u8) -> Operation {
    Op::Set { slot, stack, count }
}

#[derive(Debug)]
pub struct Outcome {
    pub result: Result<(), Rejected>,
    pub after: Vec<Stack>,
    pub dropped: Vec<Planned<u16>>,
}

pub fn request_in(
    scratch: &mut Plan<usize, u16>,
    slots: &[Slot],
    ops: impl IntoIterator<Item = Operation>,
) -> Outcome {
    let result = mcrs_minecraft_inventory_transaction::plan(
        ops,
        scratch,
        |index| {
            let slot = slots.get(index)?;
            Some(Loaded {
                stack: slot.stack,
                limit: slot.limit,
                take: slot.take,
            })
        },
        |index, handle| {
            if slots[index].refuses == Some(components(handle)) {
                Filter::Refuses
            } else {
                Filter::Permits
            }
        },
        |left, right| components(left) == components(right),
    );
    let mut after: Vec<Stack> = slots.iter().map(|slot| slot.stack).collect();
    for entry in scratch.changes() {
        assert_eq!(
            entry.before, slots[entry.slot].stack,
            "a change states the stack that the slot held"
        );
        after[entry.slot] = entry.after;
    }
    Outcome {
        result,
        after,
        dropped: scratch.dropped().collect(),
    }
}

pub fn request(slots: &[Slot], ops: &[Operation]) -> Outcome {
    request_in(&mut Plan::new(), slots, ops.iter().copied())
}
