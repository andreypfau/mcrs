use alloc::vec::Vec;

use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_table::ItemTable;

use crate::Removed;
use crate::item_components::ItemComponents;
use crate::row::Row;
use crate::row::bit;
use crate::undecoded::Undecoded;

/// Makes the entity a stack equal to `value`. It despawns the entity when the table has no
/// prototype of the kind.
// chisle: each component is one insert and one archetype move; a bundle built by component ids
// lifts it when the spawn cost shows in a profile.
pub(crate) fn spawn_stack(mut entity: EntityWorldMut, value: ItemStack) {
    let prototype =
        (entity.world().get_resource::<ItemTable>()).and_then(|table| table.prototype(value.kind));
    let Some(prototype) = prototype else {
        entity.despawn();
        return;
    };
    entity.world_scope(|world| world.init_resource::<ItemComponents>());
    entity.insert((value.kind, value.count));
    let mut patched = 0;
    let mut removed: Option<Removed> = None;
    let mut leftovers = Vec::new();
    for entry in value.patch() {
        let key = entry.builtin_key();
        if let Some(key) = key {
            patched |= bit(key.index());
        }
        let row = match (entry, key) {
            (PatchEntry::Custom(custom), _) => row_of(&entity, |rows| rows.custom(custom.key)),
            (PatchEntry::Value(_) | PatchEntry::Removal(_), Some(key)) => {
                row_of(&entity, |rows| rows.builtin(key))
            }
            _ => None,
        };
        match (entry, row) {
            (PatchEntry::Removal(key), Some(row)) => {
                let in_prototype = (prototype.components().iter()).any(|value| value.key() == *key);
                if row.is_neutral() && in_prototype {
                    removed = Some(removed.map_or(Removed::of(*key), |removed| removed.with(*key)));
                }
            }
            (PatchEntry::Removal(_), None) => {}
            (_, Some(row)) if row.insert_entry(&mut entity, entry) => {}
            _ => leftovers.push(entry.clone()),
        }
    }
    for component in prototype.components() {
        let key = component.key();
        if patched & bit(key.index()) == 0
            && let Some(row) = row_of(&entity, |rows| rows.builtin(key))
        {
            row.insert_static(&mut entity, component);
        }
    }
    if let Some(removed) = removed {
        entity.insert(removed);
    }
    if !leftovers.is_empty() {
        entity.insert(Undecoded(leftovers.into_boxed_slice()));
    }
}

fn row_of(
    entity: &EntityWorldMut,
    find: impl FnOnce(&ItemComponents) -> Option<&Row>,
) -> Option<Row> {
    (entity.world().get_resource::<ItemComponents>())
        .and_then(find)
        .copied()
}
