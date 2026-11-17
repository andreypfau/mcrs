use bevy_ecs::system::{Res, ResMut};
use mcrs_minecraft_entity_rows::RowStore;

use crate::row_attributes::RowAttributes;

pub(crate) fn forget_removed_rows<M: Send + Sync + 'static>(
    rows: Res<RowStore<M>>,
    mut attributes: ResMut<RowAttributes>,
) {
    for removed in rows.changes().removed() {
        attributes.sets.remove(&removed.id);
    }
}

pub(crate) fn any_row_attributes(attributes: Res<RowAttributes>) -> bool {
    !attributes.sets.is_empty()
}
