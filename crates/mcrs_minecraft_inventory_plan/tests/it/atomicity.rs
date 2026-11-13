use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Plan;
use mcrs_minecraft_inventory_plan::Planned;
use mcrs_minecraft_inventory_plan::Reason;
use mcrs_minecraft_inventory_plan::Rejected;

use crate::holder::Operation;
use crate::holder::Slot;
use crate::holder::Stack;
use crate::holder::components;
use crate::holder::dirt;
use crate::holder::drop_from;
use crate::holder::more_stone;
use crate::holder::named_stone;
use crate::holder::open;
use crate::holder::pearl;
use crate::holder::request_in;
use crate::holder::split;
use crate::holder::stone;

#[test]
fn a_rejected_last_operation_leaves_every_slot_and_gives_no_dropped_stack() {
    let slots = [open(stone(10)), open(None)];
    let before: Vec<Stack> = slots.iter().map(|slot| slot.stack).collect();
    let mut scratch = Plan::new();

    let applied = request_in(&mut scratch, &slots, [split(0, 1, 4), drop_from(1, 1)]);
    assert_eq!(applied.result, Ok(()));
    assert_eq!(applied.dropped.len(), 1);

    let ops = [split(0, 1, 4), drop_from(1, 1), drop_from(0, 7)];
    let rejected = request_in(&mut scratch, &slots, ops);
    let reason = Reason::NotEnough;
    assert_eq!(rejected.result, Err(Rejected { op: 2, reason }));
    assert_eq!(rejected.after, before);
    assert_eq!(rejected.dropped, []);
}

struct Random(u64);

impl Random {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }

    fn pick<V: Copy>(&mut self, values: &[V]) -> V {
        values[self.below(values.len() as u64) as usize]
    }
}

const SLOTS: usize = 6;

fn random_slot(random: &mut Random) -> Slot {
    let limit: u8 = random.pick(&[99, 99, 64, 5, 1]);
    let full: [Stack; 6] = [
        None,
        stone(64),
        more_stone(64),
        named_stone(64),
        dirt(64),
        pearl(16),
    ];
    let stack = random.pick(&full).map(|stack| {
        let most = u64::from(stack.count.min(limit));
        Planned {
            count: 1 + random.below(most) as u8,
            ..stack
        }
    });
    let take = random.pick(&[
        Filter::Permits,
        Filter::Permits,
        Filter::Permits,
        Filter::Refuses,
    ]);
    let refuses = random.pick(&[None, None, None, Some(1), Some(2)]);
    Slot {
        stack,
        limit,
        take,
        refuses,
    }
}

fn random_op(random: &mut Random) -> Operation {
    let from = random.below(SLOTS as u64) as usize;
    let to = random.below(SLOTS as u64) as usize;
    let count = random.below(12) as u8;
    match random.below(4) {
        0 => Op::Split { from, to, count },
        1 => Op::Merge { from, to },
        2 => Op::Swap {
            left: from,
            right: to,
        },
        _ => Op::Drop { from, count },
    }
}

fn total(stacks: impl IntoIterator<Item = Stack>, wanted: u16) -> u32 {
    let held = stacks.into_iter().flatten();
    held.filter(|stack| components(stack.stack) == wanted)
        .map(|stack| u32::from(stack.count))
        .sum()
}

#[test]
fn random_requests_keep_the_items_or_change_nothing() {
    let mut random = Random(0x5eed);
    let mut scratch = Plan::new();
    let mut applied = 0;
    let mut rejected = 0;
    for round in 0..4000 {
        let slots: Vec<Slot> = (0..SLOTS).map(|_| random_slot(&mut random)).collect();
        let length = 1 + random.below(4);
        let ops: Vec<Operation> = (0..length).map(|_| random_op(&mut random)).collect();
        let before: Vec<Stack> = slots.iter().map(|slot| slot.stack).collect();
        let outcome = request_in(&mut scratch, &slots, ops.iter().copied());
        if outcome.result.is_err() {
            rejected += 1;
            assert_eq!(outcome.after, before, "round {round}: {ops:?}");
            assert_eq!(outcome.dropped, [], "round {round}: {ops:?}");
            continue;
        }
        applied += 1;
        for wanted in 1..=4 {
            let dropped = outcome.dropped.iter().copied().map(Some);
            assert_eq!(
                total(before.iter().copied(), wanted),
                total(outcome.after.iter().copied().chain(dropped), wanted),
                "round {round}: {ops:?} over {slots:?}"
            );
        }
        for (slot, stack) in slots.iter().zip(&outcome.after) {
            let count = stack.map_or(0, |stack| stack.count);
            let max = stack.map_or(99, |stack| stack.max);
            assert!(
                count <= slot.limit.min(max),
                "round {round}: {ops:?} put {count} items in {slot:?}"
            );
        }
    }
    assert!(
        applied > 200 && rejected > 200,
        "the generator gives both results often: {applied} applied, {rejected} rejected"
    );
}
