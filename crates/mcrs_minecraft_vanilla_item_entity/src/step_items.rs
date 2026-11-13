use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use glam::DVec3;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Lifetime;
use mcrs_minecraft_item_entity::merge_due;
use mcrs_minecraft_physics::block_at;

use crate::item_row::ItemRow;

/// How far below the bottom of the dimension an item entity is removed (IEN-MOV-004).
const VOID_DEPTH: f64 = 64.0;

/// The item entities that live after the counters of the tick, by UUID, with their positions
/// after the pass. The positions reach [`Dropped`] only after the tile moves, so that a row is
/// found in the tile of its `Dropped` position at every other point of the tick.
#[derive(Resource, Debug, Default)]
pub(crate) struct Stepped(pub(crate) Vec<SteppedItem>);

#[derive(Clone, Copy, Debug)]
pub(crate) struct SteppedItem {
    pub(crate) stack: Entity,
    pub(crate) key: EntityKey,
    pub(crate) position: DVec3,
    /// The item looks for merge partners in this tick (IEN-MRG-001).
    pub(crate) merges: bool,
}

/// Reads the row of each item entity after the pass, runs its counters, and removes it at the
/// end of its lifetime or below the world (IEN-MOV-002, IEN-AGE-001, IEN-MOV-004).
pub(crate) fn step_items<M: Send + Sync + 'static>(
    mut commands: Commands,
    mut items: Query<(Entity, &Dropped, &mut ItemEntity, &ItemRow)>,
    rows: Res<RowStore<M>>,
    dimension: Option<Single<&DimensionTypeConfig, With<Dimension>>>,
    mut stepped: ResMut<Stepped>,
) {
    stepped.0.clear();
    let floor = dimension.map_or(f64::NEG_INFINITY, |config| {
        f64::from(config.min_y) - VOID_DEPTH
    });
    for (stack, dropped, mut item, row) in &mut items {
        let at = Placed {
            tile: TileKey::at(dropped.pos),
            id: row.id,
        };
        let Some(state) = rows.state_of(at) else {
            continue;
        };
        if dropped.pos.y < floor {
            commands.entity(stack).despawn();
            continue;
        }
        item.count_down();
        let position = state.body.position;
        stepped.0.push(SteppedItem {
            stack,
            key: row.key,
            position,
            merges: merge_due(state.age, block_at(dropped.pos) != block_at(position)),
        });
    }
    stepped.0.sort_unstable_by_key(|item| item.key);
}

/// Increases the age of each item entity after its merges, and removes it at the end of its
/// lifetime: steps 8 and 10 of IEN-MOV-006. The merges read the age before this step.
pub(crate) fn age_items(
    mut commands: Commands,
    mut items: Query<(Entity, &mut ItemEntity), With<ItemRow>>,
) {
    for (stack, mut item) in &mut items {
        if item.grow() == Lifetime::Ends {
            commands.entity(stack).despawn();
        }
    }
}

/// Writes the position of each item entity after the tile moves of the rows.
pub(crate) fn project_positions(
    stepped: Res<Stepped>,
    mut items: Query<&mut Dropped, With<ItemRow>>,
) {
    for item in &stepped.0 {
        if let Ok(mut dropped) = items.get_mut(item.stack)
            && dropped.pos != item.position
        {
            dropped.pos = item.position;
        }
    }
}
