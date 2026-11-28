use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::In;
use bevy_ecs::system::InMut;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_inventory_transaction::Loaded;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_inventory_transaction::Plan;
use mcrs_minecraft_inventory_transaction::Planned;
use mcrs_minecraft_inventory_transaction::plan;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;

use crate::Gate;
use crate::GateQuery;
use crate::InSlot;
use crate::Refusal;
use crate::Rejected;
use crate::Request;
use crate::RequestCheck;
use crate::RequestDrop;
use crate::RequestId;
use crate::RequestKind;
use crate::RequestResult;
use crate::SlotAccess;
use crate::SlotChanged;
use crate::SlotPolicy;
use crate::SlotRef;
use crate::Slots;
use crate::StackRows;
use crate::Stacks;
use crate::capacity::capacity;
use crate::placement::Placement;
use crate::request_scratch::RequestScratch;
use crate::settlement::Placed;
use crate::settlement::Settlement;
use crate::settlement::Source;
use crate::settlement::Taken;
use crate::slot_changed::write_slot_changed;
use crate::spawn_stack::spawn_stack;

#[derive(Default)]
struct PlanBuffers {
    ops: Vec<Op<SlotRef, Planned<Entity>>>,
    plan: Box<Plan<SlotRef, Entity>>,
}

pub(crate) fn commit(world: &mut World, actor: Entity, request: Request) {
    let id = request.id();
    let kind = request.kind();
    let mut check = RequestCheck::new(actor, request);
    if kind == RequestKind::Action {
        world.trigger_ref(&mut check);
    }
    let outcome = match check.denial() {
        Some(denial) => Err(Refusal::Denied(denial)),
        None => {
            world.flush();
            let Some(outcome) = plan_and_apply(world, actor, check.request()) else {
                return;
            };
            outcome.map_err(Refusal::Rejected)
        }
    };
    let result = RequestResult {
        holder: actor,
        id,
        outcome,
    };
    world.write_message(result);
    world.trigger(result);
}

fn plan_and_apply(
    world: &mut World,
    actor: Entity,
    request: &Request,
) -> Option<Result<(), Rejected>> {
    let mut settlement = world
        .get_resource_mut::<RequestScratch>()
        .map(|mut scratch| mem::take(&mut scratch.settlement))
        .unwrap_or_default();
    let planned = world.run_system_cached_with(plan_request, (request, actor, &mut settlement));
    if let Ok(Ok(())) = planned {
        apply(world, actor, request.id(), &mut settlement);
    }
    settlement.clear();
    if let Some(mut scratch) = world.get_resource_mut::<RequestScratch>() {
        scratch.settlement = settlement;
    }
    planned.ok()
}

fn plan_request(
    (InRef(request), In(actor), InMut(settlement)): (InRef<Request>, In<Entity>, InMut<Settlement>),
    world: &World,
    holders: Query<(&Slots, Option<&SlotPolicy>)>,
    mut stacks: Stacks,
    mut rows: StackRows,
    mut buffers: Local<PlanBuffers>,
) -> Result<(), Rejected> {
    let buffers = &mut *buffers;
    buffers.ops.clear();
    for op in request.ops() {
        buffers.ops.push(resolve(op, &mut stacks));
    }
    let load = |slot: SlotRef| {
        let (slots, policy) = holders.get(slot.holder).ok()?;
        let index = usize::from(slot.index);
        if index >= slots.len() {
            return None;
        }
        let stack = (slots.get(index)).and_then(|stack| planned(&mut stacks, stack));
        let access = access(policy, slot);
        let take = match (access.take, stack) {
            _ if request.kind() == RequestKind::Close => Filter::Permits,
            (Gate::Permits, _) => Filter::Permits,
            (Gate::Refuses, _) => Filter::Refuses,
            (Gate::Ask(_), None) => Filter::Permits,
            (gate @ Gate::Ask(_), Some(stack)) => {
                let query = GateQuery {
                    slot,
                    actor,
                    stack: stack.stack,
                };
                gate.answer(world, &query)
            }
        };
        Some(Loaded {
            stack,
            limit: access.limit,
            take,
        })
    };
    let place = |slot: SlotRef, stack: Entity| {
        let policy = holders.get(slot.holder).ok().and_then(|(_, policy)| policy);
        let access = access(policy, slot);
        let query = GateQuery { slot, actor, stack };
        access.place.answer(world, &query)
    };
    let equal = |left, right| rows.mergeable(left, right);
    let outcome = plan(
        buffers.ops.iter().copied(),
        &mut buffers.plan,
        load,
        place,
        equal,
    );
    settlement.clear();
    if outcome.is_ok() {
        settle(&buffers.plan, settlement);
    }
    outcome
}

fn access(policy: Option<&SlotPolicy>, slot: SlotRef) -> SlotAccess {
    policy.map_or(SlotAccess::OPEN, |policy| policy.access(slot.index))
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

fn settle(plan: &Plan<SlotRef, Entity>, settlement: &mut Settlement) {
    for (index, entry) in plan.changes().enumerate() {
        settlement.changed.push(entry.slot);
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
    for dropped in plan.dropped() {
        let ending = settlement
            .despawned
            .iter()
            .position(|&stack| stack == dropped.stack);
        let source = match ending {
            Some(index) => Source::Same(settlement.despawned.swap_remove(index)),
            None => Source::CopyOf(dropped.stack),
        };
        settlement.dropped.push(Taken {
            source,
            count: dropped.count,
        });
    }
}

fn holds(stack: Option<Planned<Entity>>, entity: Entity) -> bool {
    stack.is_some_and(|stack| stack.stack == entity)
}

fn claimant(plan: &Plan<SlotRef, Entity>, stack: Entity) -> Option<usize> {
    let mut changes = plan.changes().enumerate();
    let (home, entry) = changes.find(|(_, entry)| holds(entry.before, stack))?;
    if holds(entry.after, stack) {
        return Some(home);
    }
    plan.changes().position(|entry| holds(entry.after, stack))
}

fn apply(world: &mut World, holder: Entity, id: RequestId, settlement: &mut Settlement) {
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
    for index in 0..settlement.dropped.len() {
        let Some(&Taken {
            source: Source::CopyOf(original),
            count,
        }) = settlement.dropped.get(index)
        else {
            continue;
        };
        let reused = |source: &Source| matches!(*source, Source::Same(same) if same == original);
        let unplaced = Placement::of(world, original) == Some(Placement::Nowhere)
            && !settlement
                .placed
                .iter()
                .any(|placed| reused(&placed.source))
            && !(settlement.dropped.iter().take(index)).any(|earlier| reused(&earlier.source));
        let stack = if unplaced {
            Some(original)
        } else {
            spawn_copy(world, original, count)
        };
        if let (Some(stack), Some(dropped)) = (stack, settlement.dropped.get_mut(index)) {
            dropped.source = Source::Same(stack);
        }
    }
    for &stack in &settlement.despawned {
        if let Ok(entity) = world.get_entity_mut(stack) {
            entity.despawn();
        }
    }
    for dropped in &settlement.dropped {
        let Source::Same(stack) = dropped.source else {
            continue;
        };
        set_count(world, stack, dropped.count);
        Placement::Nowhere.apply(world, stack);
    }
    for placed in &settlement.placed {
        let Source::Same(stack) = placed.source else {
            continue;
        };
        set_count(world, stack, placed.count);
        let slot = InSlot::new(placed.slot.holder, placed.slot.index);
        Placement::Slot(slot).apply(world, stack);
    }
    for &slot in &settlement.changed {
        write_slot_changed(world.get_resource_mut::<Messages<SlotChanged>>(), slot);
    }
    for dropped in &settlement.dropped {
        if let Source::Same(stack) = dropped.source {
            world.write_message(RequestDrop { holder, id, stack });
        }
    }
}

fn set_count(world: &mut World, stack: Entity, count: u8) {
    if let (Some(count), Some(mut current)) = (Count::new(count), world.get_mut::<Count>(stack)) {
        current.set_if_neq(count);
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
