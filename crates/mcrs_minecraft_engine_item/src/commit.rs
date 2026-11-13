use alloc::vec::Vec;
use core::mem;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::In;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Loaded;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Plan;
use mcrs_minecraft_inventory_plan::Planned;
use mcrs_minecraft_inventory_plan::plan;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;

use crate::InSlot;
use crate::Rejected;
use crate::Request;
use crate::RequestResult;
use crate::SlotRef;
use crate::Slots;
use crate::StackRows;
use crate::Stacks;
use crate::capacity::SLOT_LIMIT;
use crate::capacity::capacity;
use crate::placement::Placement;
use crate::request_scratch::RequestScratch;
use crate::spawn_stack::spawn_stack;

/// What an applied request does to the entities: the stack of each changed slot that is not
/// empty after it, and the stacks that end.
#[derive(Default)]
pub(crate) struct Settlement {
    placed: Vec<Placed>,
    despawned: Vec<Entity>,
}

impl Settlement {
    fn clear(&mut self) {
        self.placed.clear();
        self.despawned.clear();
    }
}

struct Placed {
    slot: SlotRef,
    source: Source,
    count: u8,
}

#[derive(Clone, Copy)]
enum Source {
    /// The entity itself goes to the slot.
    Same(Entity),
    /// A new entity with the item and the components of this one goes to the slot. A stack with
    /// no place goes to the first such slot itself, so that a set does not leave it behind.
    CopyOf(Entity),
}

/// Plans `request` and, when it applies, changes the stacks to its result. Writes and triggers
/// [`RequestResult`] for `holder`.
pub(crate) fn commit(world: &mut World, holder: Entity, request: &Request) {
    let Ok(outcome) = world.run_system_cached_with(plan_request, request) else {
        return;
    };
    if outcome.is_ok() {
        apply(world);
    }
    let result = RequestResult {
        holder,
        id: request.id(),
        outcome,
    };
    world.write_message(result);
    world.trigger(result);
}

fn plan_request(
    InRef(request): InRef<Request>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    mut rows: StackRows,
    mut scratch: ResMut<RequestScratch>,
) -> Result<(), Rejected> {
    let scratch = &mut *scratch;
    scratch.ops.clear();
    for op in request.ops() {
        scratch.ops.push(resolve(op, &mut stacks));
    }
    let load = |slot: SlotRef| {
        let slots = holders.get(slot.holder).ok()?;
        let index = usize::from(slot.index);
        if index >= slots.len() {
            return None;
        }
        let stack = (slots.get(index)).and_then(|stack| planned(&mut stacks, stack));
        Some(Loaded {
            stack,
            limit: SLOT_LIMIT,
            take: Filter::Permits,
        })
    };
    let permits = |_, _| Filter::Permits;
    let equal = |left, right| rows.mergeable(left, right);
    let outcome = plan(
        scratch.ops.iter().copied(),
        &mut scratch.plan,
        load,
        permits,
        equal,
    );
    scratch.settlement.clear();
    if outcome.is_ok() {
        settle(&scratch.plan, &mut scratch.settlement);
    }
    outcome
}

fn resolve(op: Op<SlotRef, Entity>, stacks: &mut Stacks) -> Op<SlotRef, Planned<Entity>> {
    match op {
        Op::Split { from, to, count } => Op::Split { from, to, count },
        Op::Merge { from, to } => Op::Merge { from, to },
        Op::Swap { left, right } => Op::Swap { left, right },
        Op::Drop { from, count } => Op::Drop { from, count },
        Op::Set { slot, stack, count } => Op::Set {
            slot,
            stack: stack.and_then(|stack| planned(stacks, stack)),
            count,
        },
    }
}

fn planned(stacks: &mut Stacks, stack: Entity) -> Option<Planned<Entity>> {
    let kind = stacks.kind(stack)?;
    let count = stacks.count(stack)?;
    Some(Planned {
        stack,
        kind: kind.0,
        count: count.get(),
        max: capacity(stacks, stack),
    })
}

/// A stack ends when no changed slot holds it after the request. A changed slot holds its own
/// stack when it is the claimant of that stack, and a new copy in the other case.
fn settle(plan: &Plan<SlotRef, Entity>, settlement: &mut Settlement) {
    for (index, entry) in plan.changes().enumerate() {
        if let Some(before) = entry.before
            && !(plan.changes()).any(|other| holds(other.after, before.stack))
        {
            settlement.despawned.push(before.stack);
        }
        let Some(after) = entry.after else {
            continue;
        };
        let source = if claimant(plan, after.stack) == Some(index) {
            Source::Same(after.stack)
        } else {
            Source::CopyOf(after.stack)
        };
        settlement.placed.push(Placed {
            slot: entry.slot,
            source,
            count: after.count,
        });
    }
}

fn holds(stack: Option<Planned<Entity>>, entity: Entity) -> bool {
    stack.is_some_and(|stack| stack.stack == entity)
}

/// The changed slot that keeps the entity `stack`: its own slot when that slot still holds it,
/// else the first changed slot that holds it. `None` when its own slot did not change, because
/// that slot keeps it.
fn claimant(plan: &Plan<SlotRef, Entity>, stack: Entity) -> Option<usize> {
    let mut changes = plan.changes().enumerate();
    let (home, entry) = changes.find(|(_, entry)| holds(entry.before, stack))?;
    if holds(entry.after, stack) {
        return Some(home);
    }
    plan.changes().position(|entry| holds(entry.after, stack))
}

fn apply(world: &mut World) {
    let Some(mut scratch) = world.get_resource_mut::<RequestScratch>() else {
        return;
    };
    let mut settlement = mem::take(&mut scratch.settlement);
    for index in 0..settlement.placed.len() {
        let Some(&Placed {
            source: Source::CopyOf(original),
            count,
            ..
        }) = settlement.placed.get(index)
        else {
            continue;
        };
        let mut earlier = settlement.placed.iter().take(index);
        let unplaced = Placement::of(world, original) == Some(Placement::Nowhere)
            && !earlier
                .any(|placed| matches!(placed.source, Source::Same(same) if same == original));
        let stack = if unplaced {
            Some(original)
        } else {
            spawn_copy(world, original, count)
        };
        if let (Some(stack), Some(placed)) = (stack, settlement.placed.get_mut(index)) {
            placed.source = Source::Same(stack);
        }
    }
    for &stack in &settlement.despawned {
        if let Ok(entity) = world.get_entity_mut(stack) {
            entity.despawn();
        }
    }
    for placed in &settlement.placed {
        let Source::Same(stack) = placed.source else {
            continue;
        };
        if let (Some(count), Some(mut current)) =
            (Count::new(placed.count), world.get_mut::<Count>(stack))
        {
            current.set_if_neq(count);
        }
        let slot = InSlot::new(placed.slot.holder, placed.slot.index);
        Placement::Slot(slot).apply(world, stack);
    }
    settlement.clear();
    if let Some(mut scratch) = world.get_resource_mut::<RequestScratch>() {
        scratch.settlement = settlement;
    }
}

fn spawn_copy(world: &mut World, original: Entity, count: u8) -> Option<Entity> {
    let mut value = (world.run_system_cached_with(value_of, original).ok())??;
    value.count = Count::new(count)?;
    let entity = world.spawn_empty();
    let copy = entity.id();
    spawn_stack(entity, value);
    Some(copy)
}

fn value_of(In(stack): In<Entity>, mut stacks: Stacks) -> Option<ItemStack> {
    stacks.to_value(stack)
}
