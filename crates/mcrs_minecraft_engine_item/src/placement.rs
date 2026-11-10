use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_item_stack::Kind;

use crate::Dropped;
use crate::InSlot;
use crate::Slots;

/// The place of a stack. A stack has at most one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Placement {
    Slot(InSlot),
    World(Dropped),
    Nowhere,
}

impl Placement {
    /// The placement of `stack`, or `None` when it is not a stack.
    pub(crate) fn of(world: &World, stack: Entity) -> Option<Self> {
        let entity = world.get_entity(stack).ok()?;
        entity.get::<Kind>()?;
        let placement = match (entity.get::<InSlot>(), entity.get::<Dropped>()) {
            (Some(slot), _) => Self::Slot(*slot),
            (None, Some(dropped)) => Self::World(*dropped),
            (None, None) => Self::Nowhere,
        };
        Some(placement)
    }

    /// Gives `stack` this placement in place of the one that it has. It does nothing to an entity
    /// that is not a stack.
    pub(crate) fn apply(self, world: &mut World, stack: Entity) {
        let Ok(mut entity) = world.get_entity_mut(stack) else {
            return;
        };
        if !entity.contains::<Kind>() {
            return;
        }
        match self {
            Self::Slot(slot) => {
                if entity.get::<InSlot>() != Some(&slot) {
                    entity.remove::<Dropped>().insert(slot);
                }
            }
            Self::World(dropped) => {
                entity.remove::<InSlot>().insert(dropped);
            }
            Self::Nowhere => {
                entity.remove::<(InSlot, Dropped)>();
            }
        }
    }
}

/// Exchanges the placements of two stacks. It does nothing when one of them is not a stack.
pub(crate) fn swap(world: &mut World, stack: Entity, other: Entity) {
    if stack == other {
        return;
    }
    let (Some(mine), Some(theirs)) = (Placement::of(world, stack), Placement::of(world, other))
    else {
        return;
    };
    theirs.apply(world, stack);
    mine.apply(world, other);
}

/// Moves `stack` to the slot `index` of `holder`. It does nothing when the slot does not exist.
pub(crate) fn put_at(world: &mut World, stack: Entity, holder: Entity, index: u16) {
    let exists = (world.get::<Slots>(holder)).is_some_and(|slots| usize::from(index) < slots.len());
    if exists {
        Placement::Slot(InSlot::new(holder, index)).apply(world, stack);
    }
}
