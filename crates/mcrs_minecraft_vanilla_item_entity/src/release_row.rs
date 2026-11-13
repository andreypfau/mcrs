use bevy_ecs::lifecycle::Discard;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_item_entity::ItemEntity;

use crate::Owner;
use crate::Thrower;
use crate::item_row::ItemRow;
use crate::publish_items::Released;

/// Removes the row of a stack that leaves the world, moves in it or despawns. A stack dropped
/// again gets a new row at its new position.
pub(crate) fn release_row<M: Send + Sync + 'static>(
    replaced: On<Discard<Dropped>>,
    stacks: Query<(&Dropped, &ItemRow)>,
    mut rows: ResMut<RowStore<M>>,
    mut released: ResMut<Released>,
    mut commands: Commands,
) {
    let Ok((dropped, row)) = stacks.get(replaced.entity) else {
        return;
    };
    let tile = TileKey::at(dropped.pos);
    rows.despawn(tile, row.id);
    released.0.push(Placed { tile, id: row.id });
    commands.entity(replaced.entity).try_remove::<ItemRow>();
}

/// A stack that leaves the world stops being an item entity.
pub(crate) fn forget_item_values(removed: On<Remove<Dropped>>, mut commands: Commands) {
    commands
        .entity(removed.entity)
        .try_remove::<(ItemEntity, Owner, Thrower)>();
}
