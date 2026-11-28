use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Kind;

use crate::slot_changed::touch;

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
            touch(&mut entity);
        }
        None => entity.despawn(),
    }
}
