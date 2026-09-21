use bevy_ecs::entity::Entity;
use bevy_ecs::system::Command;
use bevy_ecs::world::World;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_item::dropped::{DEFAULT_HEALTH, THROWN_PICKUP_DELAY};
use mcrs_minecraft_item::value::{is_child_kind, ops};
use mcrs_minecraft_item::{
    DroppedItem, Held, ItemStack, Items, SlotTable, StackError, StackRevision, Thrower,
    max_stack_size, same_item_same_components, stack_to_value,
};
use mcrs_minecraft_protocol::item::{ItemComponentKind, ItemComponentValue, ItemStackValue};

use crate::slot::{Slot, stack_in};
use crate::value::{apply_value, spawn_stack, spawn_stack_into};

/// One step of a stack transaction. Slots address cells so that a planner
/// can refer to a stack it has split off before the entity exists; entities
/// address stacks the caller already holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Moves `count` of the stack in `from` into `to`, which is empty or holds
    /// the same item with the same components.
    Transfer {
        from: Slot,
        to: Slot,
        count: u8,
    },
    Swap {
        a: Slot,
        b: Slot,
    },
    /// Spawns a full stack of the item in `from` into the empty `to`.
    Clone {
        from: Slot,
        to: Slot,
    },
    /// Takes `count` of the stack in `from` out of its holder and throws it.
    Drop {
        from: Slot,
        count: u8,
        thrower: Entity,
    },
    Spawn {
        value: ItemStackValue,
        to: Slot,
    },
    /// Turns a prepared entity into a dropped stack.
    SpawnDropped {
        entity: Entity,
        value: ItemStackValue,
        pickup_delay: i16,
        thrower: Option<Entity>,
    },
    /// Moves a stack the caller holds into the empty `to`.
    Place {
        stack: Entity,
        to: Slot,
    },
    /// Rewrites the stack in place; the item must stay the same.
    Apply {
        stack: Entity,
        value: ItemStackValue,
    },
    Insert {
        stack: Entity,
        component: ItemComponentValue,
    },
    Remove {
        stack: Entity,
        kind: ItemComponentKind,
    },
    Despawn {
        stack: Entity,
    },
    /// Takes `count` off a dropped item into `to`, which is empty or holds the
    /// same item; the item entity despawns once it is emptied.
    Pickup {
        item: Entity,
        to: Slot,
        count: u8,
    },
    /// Folds one dropped item into another; the survivor keeps the longer
    /// pickup delay and the younger age.
    MergeDropped {
        from: Entity,
        into: Entity,
    },
}

/// The only writer of stack truth. Ops apply in order with immediate
/// visibility; the first one whose precondition fails stops the rest.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transaction(pub Vec<Op>);

impl Transaction {
    /// Applies the ops until one fails; the ops before it stay applied.
    pub fn try_apply(self, world: &mut World) -> Result<(), TransactionError> {
        let items = world.resource::<Items>().clone();
        for op in self.0 {
            apply_op(world, &items, &op).map_err(|error| {
                tracing::warn!(%error, ?op, "a stack transaction stopped");
                error
            })?;
        }
        Ok(())
    }
}

impl Command for Transaction {
    type Out = ();

    fn apply(self, world: &mut World) {
        let _ = self.try_apply(world);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("{0:?} is not a stack")]
    NotAStack(Entity),
    #[error("{0:?} is not a dropped item")]
    NotDropped(Entity),
    #[error("{0:?} is a dropped item")]
    Dropped(Entity),
    #[error("{0:?} is empty")]
    Empty(Slot),
    #[error("{0:?} holds a different item")]
    Different(Slot),
    #[error("holder {0:?} is gone")]
    HolderMissing(Entity),
    #[error("cell {index} of {holder:?} is occupied by {by:?}")]
    Occupied {
        holder: Entity,
        index: u16,
        by: Entity,
    },
    #[error("cell {index} is outside {holder:?}")]
    OutOfRange { holder: Entity, index: u16 },
    #[error("{0:?} would hold itself")]
    Cycle(Entity),
    #[error("{0} is derived from child stacks")]
    ChildKind(ItemComponentKind),
    #[error(transparent)]
    Stack(#[from] StackError),
}

fn apply_op(world: &mut World, items: &Items, op: &Op) -> Result<(), TransactionError> {
    match *op {
        Op::Transfer { from, to, count } => {
            let source = stack_in(world, from).ok_or(TransactionError::Empty(from))?;
            let count = count.min(count_of(world, source));
            if count == 0 {
                return Ok(());
            }
            transfer(world, items, source, to, count)
        }
        Op::Swap { a, b } => {
            let (stack_a, stack_b) = (stack_in(world, a), stack_in(world, b));
            for stack in [stack_a, stack_b].into_iter().flatten() {
                detach(world, stack);
            }
            if let Some(stack) = stack_a {
                move_stack(world, stack, b)?;
            }
            if let Some(stack) = stack_b {
                move_stack(world, stack, a)?;
            }
            Ok(())
        }
        Op::Clone { from, to } => {
            let source = stack_in(world, from).ok_or(TransactionError::Empty(from))?;
            let mut value = stack_to_value(world, source, items);
            value.count = Bounded(i32::from(max_stack_size(world.entity(source))));
            let clone = spawn_stack(world, &value, items)?;
            place_or_despawn(world, clone, to)
        }
        Op::Drop {
            from,
            count,
            thrower,
        } => {
            let source = stack_in(world, from).ok_or(TransactionError::Empty(from))?;
            let Some(thrown) = take(world, items, source, count) else {
                return Ok(());
            };
            detach(world, thrown);
            drop_stack(world, thrown, THROWN_PICKUP_DELAY, Some(thrower));
            Ok(())
        }
        Op::Spawn { ref value, to } => {
            let stack = spawn_stack(world, value, items)?;
            place_or_despawn(world, stack, to)
        }
        Op::SpawnDropped {
            entity,
            ref value,
            pickup_delay,
            thrower,
        } => {
            spawn_stack_into(world, entity, value, items)?;
            drop_stack(world, entity, pickup_delay, thrower);
            Ok(())
        }
        Op::Place { stack, to } => move_stack(world, stack, to),
        Op::Apply { stack, ref value } => Ok(apply_value(world, stack, value, items)?),
        Op::Insert {
            stack,
            ref component,
        } => {
            let kind = component.kind();
            if is_child_kind(kind) {
                return Err(TransactionError::ChildKind(kind));
            }
            let mut entity = stack_entity(world, stack)?;
            (ops(kind).insert)(&mut entity, component);
            bump(world, stack);
            Ok(())
        }
        Op::Remove { stack, kind } => {
            if is_child_kind(kind) {
                return Err(TransactionError::ChildKind(kind));
            }
            let mut entity = stack_entity(world, stack)?;
            (ops(kind).remove)(&mut entity);
            bump(world, stack);
            Ok(())
        }
        Op::Despawn { stack } => {
            stack_entity(world, stack)?;
            set_count(world, stack, 0);
            Ok(())
        }
        Op::Pickup { item, to, count } => {
            // Another player may have taken the item earlier this tick.
            let Ok(entity) = world.get_entity(item) else {
                return Ok(());
            };
            if !entity.contains::<DroppedItem>() {
                return Err(TransactionError::NotDropped(item));
            }
            let count = count.min(count_of(world, item));
            let Some(taken) = split(world, items, item, count) else {
                return Ok(());
            };
            if let Err(error) = transfer(world, items, taken, to, count) {
                // The item stays whole when its cell was taken meanwhile.
                if world.get_entity(item).is_ok() {
                    merge_into(world, taken, item, u8::MAX);
                }
                if world.get::<ItemStack>(taken).is_some() {
                    world.despawn(taken);
                }
                return Err(error);
            }
            Ok(())
        }
        Op::MergeDropped { from, into } => {
            let (Some(from_item), Some(into_item)) = (
                world.get::<DroppedItem>(from).copied(),
                world.get::<DroppedItem>(into).copied(),
            ) else {
                return Err(TransactionError::NotDropped(from));
            };
            let max = max_stack_size(world.entity(into));
            if usize::from(count_of(world, from)) + usize::from(count_of(world, into))
                > usize::from(max)
                || !same_item_same_components(world, from, into, items)
            {
                return Ok(());
            }
            merge_into(world, from, into, max);
            let mut merged = world.get_mut::<DroppedItem>(into).unwrap();
            merged.pickup_delay = into_item.pickup_delay.max(from_item.pickup_delay);
            merged.age = into_item.age.min(from_item.age);
            Ok(())
        }
    }
}

fn stack_entity(
    world: &mut World,
    stack: Entity,
) -> Result<bevy_ecs::world::EntityWorldMut<'_>, TransactionError> {
    match world.get_entity_mut(stack) {
        Ok(entity) if entity.contains::<ItemStack>() => Ok(entity),
        _ => Err(TransactionError::NotAStack(stack)),
    }
}

fn count_of(world: &World, stack: Entity) -> u8 {
    world.get::<ItemStack>(stack).map_or(0, |stack| stack.count)
}

/// The stack itself when all of it is taken, else the split-off part.
fn take(world: &mut World, items: &Items, stack: Entity, count: u8) -> Option<Entity> {
    if count >= count_of(world, stack) {
        Some(stack)
    } else {
        split(world, items, stack, count)
    }
}

fn transfer(
    world: &mut World,
    items: &Items,
    source: Entity,
    to: Slot,
    count: u8,
) -> Result<(), TransactionError> {
    match stack_in(world, to) {
        None => {
            let moving = take(world, items, source, count).expect("a positive count splits");
            move_stack(world, moving, to)
        }
        Some(target) if target == source => Ok(()),
        Some(target) => {
            if !same_item_same_components(world, source, target, items) {
                return Err(TransactionError::Different(to));
            }
            merge_into(
                world,
                source,
                target,
                count_of(world, target).saturating_add(count),
            );
            Ok(())
        }
    }
}

fn place_or_despawn(world: &mut World, stack: Entity, to: Slot) -> Result<(), TransactionError> {
    move_stack(world, stack, to).inspect_err(|_| {
        world.despawn(stack);
    })
}

fn drop_stack(world: &mut World, stack: Entity, pickup_delay: i16, thrower: Option<Entity>) {
    let mut entity = world.entity_mut(stack);
    entity.insert(DroppedItem {
        age: 0,
        pickup_delay,
        health: DEFAULT_HEALTH,
    });
    if let Some(thrower) = thrower {
        entity.insert(Thrower(thrower));
    }
    bump(world, stack);
}

fn set_count(world: &mut World, stack: Entity, count: u8) {
    if count == 0 {
        bump(world, stack);
        world.despawn(stack);
        return;
    }
    world.get_mut::<ItemStack>(stack).unwrap().count = count;
    bump(world, stack);
}

fn move_stack(world: &mut World, stack: Entity, to: Slot) -> Result<(), TransactionError> {
    let Slot { holder, index } = to;
    if stack_entity(world, stack)?.contains::<DroppedItem>() {
        return Err(TransactionError::Dropped(stack));
    }
    let mut ancestor = holder;
    loop {
        if ancestor == stack {
            return Err(TransactionError::Cycle(stack));
        }
        match world.get::<Held>(ancestor) {
            Some(held) => ancestor = held.holder,
            None => break,
        }
    }
    let table = world
        .get::<SlotTable>(holder)
        .ok_or(TransactionError::HolderMissing(holder))?;
    if !table.accepts(index) {
        return Err(TransactionError::OutOfRange { holder, index });
    }
    match table.get(index) {
        Some(by) if by == stack => return Ok(()),
        Some(by) => return Err(TransactionError::Occupied { holder, index, by }),
        None => {}
    }
    let old = world.get::<Held>(stack).copied();
    attach(world, stack, holder, index);
    if let Some(old) = old {
        bump_holder(world, old.holder);
    }
    bump(world, stack);
    Ok(())
}

pub(crate) fn attach(world: &mut World, stack: Entity, holder: Entity, index: u16) {
    world.entity_mut(stack).insert(Held { holder, index });
}

fn detach(world: &mut World, stack: Entity) {
    let Some(old) = world.get::<Held>(stack).copied() else {
        return;
    };
    world.entity_mut(stack).remove::<Held>();
    touch(world, stack);
    bump_holder(world, old.holder);
}

/// `None` when nothing is taken; the source is untouched then.
fn split(world: &mut World, items: &Items, stack: Entity, count: u8) -> Option<Entity> {
    let have = count_of(world, stack);
    let taken = count.min(have);
    if taken == 0 {
        return None;
    }
    let mut value = stack_to_value(world, stack, items);
    value.count = Bounded(i32::from(taken));
    let split = spawn_stack(world, &value, items).expect("a live stack re-spawns");
    set_count(world, stack, have - taken);
    Some(split)
}

fn merge_into(world: &mut World, from: Entity, into: Entity, max: u8) -> u8 {
    if from == into {
        return 0;
    }
    let (Some(source), Some(target)) = (
        world.get::<ItemStack>(from).copied(),
        world.get::<ItemStack>(into).copied(),
    ) else {
        return 0;
    };
    let moved = source.count.min(max.saturating_sub(target.count));
    if moved == 0 {
        return 0;
    }
    set_count(world, into, target.count + moved);
    set_count(world, from, source.count - moved);
    moved
}

pub(crate) fn touch(world: &mut World, stack: Entity) {
    if let Some(mut revision) = world.get_mut::<StackRevision>(stack) {
        revision.0 += 1;
    }
}

/// Touches the stack and every stack above it, so a change deep inside a
/// container surfaces as a revision on the top-level cell's stack.
pub(crate) fn bump(world: &mut World, stack: Entity) {
    touch(world, stack);
    if let Some(held) = world.get::<Held>(stack).copied() {
        bump_holder(world, held.holder);
    }
}

fn bump_holder(world: &mut World, holder: Entity) {
    if world.get::<ItemStack>(holder).is_some() {
        bump(world, holder);
    }
}
