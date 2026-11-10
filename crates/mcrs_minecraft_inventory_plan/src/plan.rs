use crate::Entry;
use crate::Filter;
use crate::Loaded;
use crate::Op;
use crate::Planned;
use crate::Reason;
use crate::Rejected;

/// The largest number of operations in one request.
pub const MAX_OPS: usize = 128;

const MAX_SLOTS: usize = 2 * MAX_OPS;

#[derive(Clone, Copy, Debug)]
struct Tracked<I, T> {
    slot: I,
    before: Option<Planned<T>>,
    after: Option<Planned<T>>,
    limit: u8,
    take: Filter,
}

#[derive(Clone, Copy)]
struct Found<I, T> {
    slot: I,
    index: usize,
    stack: Option<Planned<T>>,
    limit: u8,
    take: Filter,
}

/// The working state and the result of one request. The caller owns it and uses it again for
/// the next request; it holds every slot and every dropped stack inline.
#[derive(Clone, Debug)]
pub struct Plan<I, T> {
    slots: [Option<Tracked<I, T>>; MAX_SLOTS],
    slot_count: usize,
    dropped: [Option<Planned<T>>; MAX_OPS],
    dropped_count: usize,
}

impl<I, T> Plan<I, T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [const { None }; MAX_SLOTS],
            slot_count: 0,
            dropped: [const { None }; MAX_OPS],
            dropped_count: 0,
        }
    }

    fn clear(&mut self) {
        self.slot_count = 0;
        self.dropped_count = 0;
    }
}

impl<I, T> Default for Plan<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Copy + PartialEq, T: Copy + PartialEq> Plan<I, T> {
    /// The slots whose stack the last applied request changed, in the order in which the
    /// request first names them. Empty after a rejection.
    pub fn changes(&self) -> impl Iterator<Item = Entry<I, T>> + '_ {
        let tracked = self.slots.iter().take(self.slot_count).flatten();
        tracked
            .filter(|tracked| tracked.before != tracked.after)
            .map(|tracked| Entry {
                slot: tracked.slot,
                before: tracked.before,
                after: tracked.after,
            })
    }

    /// The dropped stacks of the last applied request, in the order of its operations. Empty
    /// after a rejection.
    pub fn dropped(&self) -> impl Iterator<Item = Planned<T>> + '_ {
        let dropped = self.dropped.iter().take(self.dropped_count);
        dropped.flatten().copied()
    }

    fn find(
        &mut self,
        slot: I,
        load: &mut impl FnMut(I) -> Option<Loaded<T>>,
    ) -> Result<Found<I, T>, Reason> {
        let known = self.slots.iter().take(self.slot_count).flatten();
        for (index, tracked) in known.enumerate() {
            if tracked.slot == slot {
                return Ok(Found {
                    slot,
                    index,
                    stack: tracked.after,
                    limit: tracked.limit,
                    take: tracked.take,
                });
            }
        }
        let loaded = load(slot).ok_or(Reason::NoSuchSlot)?;
        let stack = loaded.stack.filter(|stack| stack.count > 0);
        let index = self.slot_count;
        let free = self.slots.get_mut(index).ok_or(Reason::TooManyOps)?;
        *free = Some(Tracked {
            slot,
            before: stack,
            after: stack,
            limit: loaded.limit,
            take: loaded.take,
        });
        self.slot_count = index.saturating_add(1);
        Ok(Found {
            slot,
            index,
            stack,
            limit: loaded.limit,
            take: loaded.take,
        })
    }

    fn set(&mut self, index: usize, stack: Option<Planned<T>>) {
        if let Some(Some(tracked)) = self.slots.get_mut(index) {
            tracked.after = stack;
        }
    }

    fn remove(&mut self, source: Found<I, T>, held: Planned<T>, count: u8) {
        let rest = held.count.saturating_sub(count);
        let stack = (rest > 0).then_some(Planned {
            count: rest,
            ..held
        });
        self.set(source.index, stack);
    }

    fn transfer(&mut self, source: Found<I, T>, target: Found<I, T>, moved: Planned<T>, count: u8) {
        self.remove(source, moved, count);
        let received = match target.stack {
            Some(held) => Planned {
                count: held.count.saturating_add(count),
                ..held
            },
            None => Planned { count, ..moved },
        };
        self.set(target.index, Some(received));
    }

    fn push_dropped(&mut self, stack: Planned<T>) -> Result<(), Reason> {
        let index = self.dropped_count;
        let free = self.dropped.get_mut(index).ok_or(Reason::TooManyOps)?;
        *free = Some(stack);
        self.dropped_count = index.saturating_add(1);
        Ok(())
    }

    fn examine(
        &mut self,
        op: Op<I>,
        load: &mut impl FnMut(I) -> Option<Loaded<T>>,
        place: &mut impl FnMut(I, T) -> Filter,
        equal: &mut impl FnMut(T, T) -> bool,
    ) -> Result<(), Reason> {
        match op {
            Op::Split { from, to, count } => {
                let source = self.find(from, load)?;
                let target = self.find(to, load)?;
                if from == to {
                    return Err(Reason::SameSlot);
                }
                if count == 0 {
                    return Err(Reason::BadCount);
                }
                let moved = source.stack.ok_or(Reason::EmptySource)?;
                if count > moved.count {
                    return Err(Reason::NotEnough);
                }
                if count > room(source, target, moved, place, equal)? {
                    return Err(Reason::NoRoom);
                }
                self.transfer(source, target, moved, count);
            }
            Op::Merge { from, to } => {
                let source = self.find(from, load)?;
                let target = self.find(to, load)?;
                if from == to {
                    return Err(Reason::SameSlot);
                }
                let moved = source.stack.ok_or(Reason::EmptySource)?;
                let count = moved.count.min(room(source, target, moved, place, equal)?);
                if count == 0 {
                    return Err(Reason::NoRoom);
                }
                self.transfer(source, target, moved, count);
            }
            Op::Swap { left, right } => {
                let first = self.find(left, load)?;
                let second = self.find(right, load)?;
                if left == right {
                    return Err(Reason::SameSlot);
                }
                admit_swap(first, second, place)?;
                self.set(first.index, second.stack);
                self.set(second.index, first.stack);
            }
            Op::Drop { from, count } => {
                let source = self.find(from, load)?;
                if count == 0 {
                    return Err(Reason::BadCount);
                }
                let held = source.stack.ok_or(Reason::EmptySource)?;
                if count > held.count {
                    return Err(Reason::NotEnough);
                }
                if source.take == Filter::Refuses {
                    return Err(Reason::TakeDenied);
                }
                self.push_dropped(Planned { count, ..held })?;
                self.remove(source, held, count);
            }
        }
        Ok(())
    }
}

fn count_of<T>(stack: Option<Planned<T>>) -> u8 {
    stack.map_or(0, |stack| stack.count)
}

/// How many items of `moved` the target can still accept, after the filters and the equality.
fn room<I: Copy, T: Copy>(
    source: Found<I, T>,
    target: Found<I, T>,
    moved: Planned<T>,
    place: &mut impl FnMut(I, T) -> Filter,
    equal: &mut impl FnMut(T, T) -> bool,
) -> Result<u8, Reason> {
    if source.take == Filter::Refuses {
        return Err(Reason::TakeDenied);
    }
    if let Some(held) = target.stack
        && !(held.kind == moved.kind && equal(moved.stack, held.stack))
    {
        return Err(Reason::NotEqual);
    }
    if place(target.slot, moved.stack) == Filter::Refuses {
        return Err(Reason::PlaceDenied);
    }
    let capacity = target.limit.min(moved.max);
    Ok(capacity.saturating_sub(count_of(target.stack)))
}

fn admit_swap<I: Copy, T: Copy>(
    first: Found<I, T>,
    second: Found<I, T>,
    place: &mut impl FnMut(I, T) -> Filter,
) -> Result<(), Reason> {
    if first.stack.is_none() && second.stack.is_none() {
        return Err(Reason::EmptySource);
    }
    for side in [first, second] {
        if side.stack.is_some() && side.take == Filter::Refuses {
            return Err(Reason::TakeDenied);
        }
    }
    for (entering, destination) in [(first.stack, second.slot), (second.stack, first.slot)] {
        if let Some(entering) = entering
            && place(destination, entering.stack) == Filter::Refuses
        {
            return Err(Reason::PlaceDenied);
        }
    }
    if count_of(first.stack) > second.limit || count_of(second.stack) > first.limit {
        return Err(Reason::NoRoom);
    }
    Ok(())
}

/// Examines `ops` in order against the working state and leaves the result in `scratch`.
///
/// `load` gives the inputs of a slot, or `None` for a slot that does not exist; the planner
/// calls it one time for each slot of a request. `place` is the place filter of a slot for a
/// stack. `equal` says whether two stacks of the same kind have the same components. `place`
/// and `equal` must give the same answer for the whole request.
///
/// When the request applies, `scratch` holds the changed slots and the dropped stacks. When it
/// is rejected, `scratch` holds nothing.
pub fn plan<I: Copy + PartialEq, T: Copy + PartialEq>(
    ops: impl IntoIterator<Item = Op<I>>,
    scratch: &mut Plan<I, T>,
    mut load: impl FnMut(I) -> Option<Loaded<T>>,
    mut place: impl FnMut(I, T) -> Filter,
    mut equal: impl FnMut(T, T) -> bool,
) -> Result<(), Rejected> {
    scratch.clear();
    let mut rejected: Option<Rejected> = None;
    for (index, op) in ops.into_iter().enumerate() {
        if index >= MAX_OPS {
            rejected = Some(Rejected {
                op: MAX_OPS,
                reason: Reason::TooManyOps,
            });
            break;
        }
        if rejected.is_some() {
            continue;
        }
        if let Err(reason) = scratch.examine(op, &mut load, &mut place, &mut equal) {
            rejected = Some(Rejected { op: index, reason });
        }
    }
    match rejected {
        Some(rejected) => {
            scratch.clear();
            Err(rejected)
        }
        None => Ok(()),
    }
}
