use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Kind;

/// Takes `items` from the count of the stack, and despawns the stack when none is left.
pub(crate) fn consume(mut entity: EntityWorldMut, items: u8) {
    if !entity.contains::<Kind>() {
        return;
    }
    let Some(&count) = entity.get::<Count>() else {
        return;
    };
    match count.get().checked_sub(items).and_then(Count::new) {
        Some(left) if left == count => {}
        Some(left) => {
            entity.insert(left);
        }
        None => entity.despawn(),
    }
}
