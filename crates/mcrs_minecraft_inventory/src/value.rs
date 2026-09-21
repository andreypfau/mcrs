use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_item::value::{
    CHILD_KINDS, child_kind, child_targets, children, container_slots, entry, item_of, named_entry,
    ops,
};
use mcrs_minecraft_item::{ItemEntry, ItemStack, Items, SlotTable, StackError, StackRevision};
use mcrs_minecraft_protocol::item::{ItemComponentKind, ItemStackValue, MAX_CHARGED_PROJECTILES};

use crate::transaction::{attach, bump, touch};

pub fn spawn_stack(
    world: &mut World,
    value: &ItemStackValue,
    items: &Items,
) -> Result<Entity, StackError> {
    let stack = world.spawn_empty().id();
    spawn_stack_into(world, stack, value, items)
        .map(|()| stack)
        .inspect_err(|_| {
            world.despawn(stack);
        })
}

/// Turns an existing entity into the stack; it must carry no stack yet.
pub fn spawn_stack_into(
    world: &mut World,
    entity: Entity,
    value: &ItemStackValue,
    items: &Items,
) -> Result<(), StackError> {
    let entry = named_entry(items, value)?;
    check(entry, value, items)?;
    spawn_checked(world, entity, entry, value, items);
    bump(world, entity);
    Ok(())
}

/// Prototype normalisation: a patch entry equal to the prototype is dropped,
/// a removal of a kind the prototype lacks is dropped, and child stacks are
/// reconciled in place. The value is checked in full before anything is
/// written, so an error leaves the world untouched.
pub fn apply_value(
    world: &mut World,
    stack: Entity,
    value: &ItemStackValue,
    items: &Items,
) -> Result<(), StackError> {
    let entry = entry(world, stack, items)?;
    if entry.identifier != *value.item.location() {
        return Err(StackError::ItemMismatch {
            stack,
            held: entry.identifier.as_str().to_owned(),
            given: value.item.as_str().to_owned(),
        });
    }
    check(entry, value, items)?;
    write_value(world, stack, entry, value, items);
    bump(world, stack);
    Ok(())
}

fn check(entry: &ItemEntry, value: &ItemStackValue, items: &Items) -> Result<(), StackError> {
    if u8::try_from(value.count.0).is_err() || value.count.0 == 0 {
        return Err(StackError::BadCount {
            item: entry.identifier.as_str().to_owned(),
            count: value.count.0,
        });
    }
    let own_child_kind = child_kind(entry);
    for kind in CHILD_KINDS {
        let Some(given) = value.components.get_value(kind) else {
            continue;
        };
        let targets = child_targets(given);
        // ponytail: vanilla lets any item carry a child-kind component; here
        // children are entities under the prototype's kind, so a foreign one
        // with contents is refused rather than kept as inert data.
        if Some(kind) != own_child_kind {
            if !targets.is_empty() {
                return Err(StackError::UnsupportedChildKind {
                    item: entry.identifier.as_str().to_owned(),
                    kind,
                });
            }
            continue;
        }
        // ponytail: vanilla keeps container slots past the block's size on the
        // item and drops them on placement; the fixed table refuses them here.
        if kind == ItemComponentKind::Container && targets.len() > container_slots(entry) {
            return Err(StackError::ContainerOverflow {
                item: entry.identifier.as_str().to_owned(),
                slots: container_slots(entry),
                needed: targets.len(),
            });
        }
        for target in targets.into_iter().flatten() {
            check(named_entry(items, target)?, target, items)?;
        }
    }
    Ok(())
}

fn spawn_checked(
    world: &mut World,
    stack: Entity,
    entry: &ItemEntry,
    value: &ItemStackValue,
    items: &Items,
) {
    world.entity_mut(stack).insert((
        ItemStack {
            item: entry.id,
            count: value.count.0 as u8,
        },
        StackRevision::default(),
    ));
    write_value(world, stack, entry, value, items);
}

fn write_value(
    world: &mut World,
    stack: Entity,
    entry: &ItemEntry,
    value: &ItemStackValue,
    items: &Items,
) {
    let own_child_kind = child_kind(entry);
    let effective = entry.prototype.apply(&value.components);
    let mut entity = world.entity_mut(stack);
    for kind in ItemComponentKind::ALL {
        if Some(kind) == own_child_kind {
            continue;
        }
        match effective.get_value(kind) {
            Some(given) => (ops(kind).insert)(&mut entity, given),
            None => (ops(kind).remove)(&mut entity),
        }
    }
    if let Some(kind) = own_child_kind {
        let removed = value.components.is_removed(kind);
        let targets = match value.components.get_value(kind) {
            Some(given) if !removed => child_targets(given),
            _ => Vec::new(),
        };
        reconcile_children(world, stack, kind, entry, &targets, items);
        // A bundle or box without its contents component refuses insertion in
        // vanilla, so the tombstone leaves no cells to move into.
        if removed {
            world.entity_mut(stack).remove::<SlotTable>();
        }
    }
    world.get_mut::<ItemStack>(stack).unwrap().count = value.count.0 as u8;
}

fn reconcile_children(
    world: &mut World,
    stack: Entity,
    kind: ItemComponentKind,
    entry: &ItemEntry,
    targets: &[Option<&ItemStackValue>],
    items: &Items,
) {
    if !world.entity(stack).contains::<SlotTable>() {
        let table = match kind {
            ItemComponentKind::Container => SlotTable::fixed(container_slots(entry)),
            ItemComponentKind::ChargedProjectiles => SlotTable::fixed(MAX_CHARGED_PROJECTILES),
            _ => SlotTable::list(),
        };
        world.entity_mut(stack).insert(table);
    }
    let existing = children(world, stack);
    for (index, (child, target)) in existing
        .iter()
        .copied()
        .chain(std::iter::repeat(None))
        .zip(targets.iter().copied().chain(std::iter::repeat(None)))
        .take(existing.len().max(targets.len()))
        .enumerate()
    {
        let target_entry =
            target.map(|target| named_entry(items, target).expect("checked before the write"));
        let kept = match (child, target_entry) {
            (Some(child), Some(target_entry)) => item_of(world, child) == Some(target_entry.id),
            (None, None) => true,
            _ => false,
        };
        if kept {
            if let (Some(child), Some(target), Some(target_entry)) = (child, target, target_entry) {
                write_value(world, child, target_entry, target, items);
                touch(world, child);
            }
            continue;
        }
        if let Some(child) = child {
            world.despawn(child);
        }
        if let (Some(target), Some(target_entry)) = (target, target_entry) {
            let child = world.spawn_empty().id();
            spawn_checked(world, child, target_entry, target, items);
            attach(world, child, stack, index as u16);
        }
    }
}
