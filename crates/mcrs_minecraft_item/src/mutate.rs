use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::{EntityWorldMut, World};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_protocol::item::ItemDataComponent;

use crate::definition::Items;
use crate::dropped::DroppedItem;
use crate::held::{Held, SlotTable};
use crate::stack::{ItemStack, StackRevision};
use crate::sync::DirtyStacks;
use crate::value::{self, stack_to_value};

pub use crate::value::{apply_value, spawn_stack};

#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("{0:?} is not a stack")]
    NotAStack(Entity),
    #[error("{0:?} is a dropped item")]
    Dropped(Entity),
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
}

pub fn set<K: ItemDataComponent + Component>(world: &mut World, stack: Entity, value: K) {
    const {
        assert!(
            !value::is_child_kind(K::KIND),
            "child kinds are derived from child stacks"
        )
    }
    let Ok(mut entity) = world.get_entity_mut(stack) else {
        return;
    };
    if !entity.contains::<ItemStack>() {
        return;
    }
    entity.insert(value);
    bump(world, stack);
}

pub fn remove<K: ItemDataComponent + Component>(world: &mut World, stack: Entity) {
    const {
        assert!(
            !value::is_child_kind(K::KIND),
            "child kinds are derived from child stacks"
        )
    }
    let Ok(mut entity) = world.get_entity_mut(stack) else {
        return;
    };
    if !entity.contains::<ItemStack>() {
        return;
    }
    entity.remove::<K>();
    bump(world, stack);
}

pub fn set_count(world: &mut World, stack: Entity, count: u8) {
    if world.get::<ItemStack>(stack).is_none() {
        return;
    }
    if count == 0 {
        bump(world, stack);
        world.despawn(stack);
        return;
    }
    world.get_mut::<ItemStack>(stack).unwrap().count = count;
    bump(world, stack);
}

pub fn move_stack(
    world: &mut World,
    stack: Entity,
    holder: Entity,
    index: u16,
) -> Result<(), MoveError> {
    let Ok(entity) = world.get_entity(stack) else {
        return Err(MoveError::NotAStack(stack));
    };
    if !entity.contains::<ItemStack>() {
        return Err(MoveError::NotAStack(stack));
    }
    if entity.contains::<DroppedItem>() {
        return Err(MoveError::Dropped(stack));
    }
    let mut ancestor = holder;
    loop {
        if ancestor == stack {
            return Err(MoveError::Cycle(stack));
        }
        match world.get::<Held>(ancestor) {
            Some(held) => ancestor = held.holder,
            None => break,
        }
    }
    let table = world
        .get::<SlotTable>(holder)
        .ok_or(MoveError::HolderMissing(holder))?;
    if !table.accepts(index) {
        return Err(MoveError::OutOfRange { holder, index });
    }
    match table.get(index) {
        Some(by) if by == stack => return Ok(()),
        Some(by) => return Err(MoveError::Occupied { holder, index, by }),
        None => {}
    }
    let old = world.get::<Held>(stack).copied();
    attach(world, stack, holder, index);
    if let Some(old) = old {
        bump_holder(world, old.holder, old.index);
    }
    bump(world, stack);
    Ok(())
}

pub(crate) fn attach(world: &mut World, stack: Entity, holder: Entity, index: u16) {
    world.entity_mut(stack).insert(Held { holder, index });
}

pub fn detach(world: &mut World, stack: Entity) {
    let Some(old) = world.get::<Held>(stack).copied() else {
        return;
    };
    world.entity_mut(stack).remove::<Held>();
    touch(world, stack);
    bump_holder(world, old.holder, old.index);
}

/// `None` when nothing is taken; the source is untouched then.
pub fn split(world: &mut World, stack: Entity, count: u8, items: &Items) -> Option<Entity> {
    let have = world.get::<ItemStack>(stack)?.count;
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

pub fn merge_into(world: &mut World, from: Entity, into: Entity, max: u8) -> u8 {
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

/// Touches the stack and every stack above it, then queues the top-level cell
/// or, for a dropped item, the stack itself; an unheld stack that is not
/// dropped has nothing to resend.
pub(crate) fn bump(world: &mut World, stack: Entity) {
    touch(world, stack);
    match world.get::<Held>(stack).copied() {
        Some(held) => bump_holder(world, held.holder, held.index),
        None if world.get::<DroppedItem>(stack).is_some() => {
            world
                .get_resource_or_init::<DirtyStacks>()
                .roots
                .push(stack);
        }
        None => {}
    }
}

fn bump_holder(world: &mut World, holder: Entity, index: u16) {
    if world.get::<ItemStack>(holder).is_some() {
        bump(world, holder);
    } else {
        world
            .get_resource_or_init::<DirtyStacks>()
            .cells
            .push((holder, index));
    }
}

pub trait StackCommands {
    fn set_component<K: ItemDataComponent + Component>(&mut self, value: K);
    fn remove_component<K: ItemDataComponent + Component>(&mut self);
    fn set_count(&mut self, count: u8);
}

impl StackCommands for EntityCommands<'_> {
    fn set_component<K: ItemDataComponent + Component>(&mut self, value: K) {
        self.queue(move |mut entity: EntityWorldMut| {
            let id = entity.id();
            entity.world_scope(|world| set(world, id, value));
        });
    }

    fn remove_component<K: ItemDataComponent + Component>(&mut self) {
        self.queue(|mut entity: EntityWorldMut| {
            let id = entity.id();
            entity.world_scope(|world| remove::<K>(world, id));
        });
    }

    fn set_count(&mut self, count: u8) {
        self.queue(move |mut entity: EntityWorldMut| {
            let id = entity.id();
            entity.world_scope(|world| set_count(world, id, count));
        });
    }
}
