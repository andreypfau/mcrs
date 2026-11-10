use bevy_ecs::component::Component;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::Prototype;

use crate::Removed;

pub(crate) enum Target<T> {
    Value(T),
    Removed,
    Prototype,
}

/// Gives the stack the effective component `target` of type `T`. It does nothing to an entity
/// without a [`Kind`].
pub(crate) fn store<T: ItemComponent + Component>(mut entity: EntityWorldMut, target: Target<T>) {
    let Some(&kind) = entity.get::<Kind>() else {
        return;
    };
    let prototype =
        (entity.world().get_resource::<ItemTable>()).and_then(|table| table.prototype(kind));
    let base = prototype.and_then(Prototype::get::<T>);
    let removal = matches!(target, Target::Removed) && base.is_some();
    let value = match target {
        Target::Value(value) => Some(value),
        Target::Removed => None,
        Target::Prototype => base.map(T::borrow_static),
    };
    match value {
        Some(value) if Some(&value) != T::neutral() => {
            entity.insert(value);
        }
        Some(_) | None => {
            entity.remove::<T>();
        }
    }
    if let Some(key) = T::builtin_key().filter(|_| T::neutral().is_some()) {
        let removed = entity.get::<Removed>().copied();
        let next = match (removed, removal) {
            (None, false) => return,
            (None, true) => Removed::of(key),
            (Some(removed), true) => removed.with(key),
            (Some(removed), false) => removed.without(key),
        };
        if next.bits() == 0 {
            if removed.is_some() {
                entity.remove::<Removed>();
            }
        } else if Some(next) != removed {
            entity.insert(next);
        }
    }
}
