use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Loaded;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Plan;
use mcrs_minecraft_inventory_plan::Planned;
use mcrs_minecraft_inventory_plan::Rejected;

pub type Stack = Option<Planned<u16>>;

pub const STONE: u16 = 1;
pub const DIRT: u16 = 2;
pub const PEARL: u16 = 3;

/// The components of a handle. Two handles with the same components are equal stacks.
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

/// A second handle with the components of [`stone`].
pub fn more_stone(count: u8) -> Stack {
    stack(11, STONE, count, 64)
}

/// Stone with a different component patch.
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
    /// The components that the place filter refuses.
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

pub fn split(from: usize, to: usize, count: u8) -> Op<usize> {
    Op::Split { from, to, count }
}

pub fn merge(from: usize, to: usize) -> Op<usize> {
    Op::Merge { from, to }
}

pub fn swap(left: usize, right: usize) -> Op<usize> {
    Op::Swap { left, right }
}

pub fn drop_from(from: usize, count: u8) -> Op<usize> {
    Op::Drop { from, count }
}

#[derive(Debug)]
pub struct Outcome {
    pub result: Result<(), Rejected>,
    pub after: Vec<Stack>,
    pub dropped: Vec<Planned<u16>>,
}

/// Plans `ops` over `slots` in `scratch`. Panics when a change names a slot that does not exist
/// or states a stack before that the slot did not hold.
pub fn request_in(
    scratch: &mut Plan<usize, u16>,
    slots: &[Slot],
    ops: impl IntoIterator<Item = Op<usize>>,
) -> Outcome {
    let result = mcrs_minecraft_inventory_plan::plan(
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

pub fn request(slots: &[Slot], ops: &[Op<usize>]) -> Outcome {
    request_in(&mut Plan::new(), slots, ops.iter().copied())
}
