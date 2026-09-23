use bevy_ecs::lifecycle::{Discard, Insert};
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Entity, Query, With, Without};
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex};

use crate::world::entity::player::column_view::ColumnView;

/// A player started or stopped holding a column on its client.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnHeld {
    pub player: Entity,
    pub dim: Entity,
    pub column: ColumnPos,
    pub held: bool,
}

/// The only writer of `PlayerObservers`. Applying the same message twice leaves the same
/// observers, so every schedule that sends columns can run it right after the send.
pub fn mirror_held_columns(
    mut held: MessageReader<ColumnHeld>,
    column_indices: Query<&ColumnIndex>,
    mut observers: Query<&mut PlayerObservers, (With<Column>, Without<Player>)>,
) {
    for change in held.read() {
        let Some(slot) = column_indices
            .get(change.dim)
            .ok()
            .and_then(|index| index.0.get(&change.column))
        else {
            continue;
        };
        let Ok(mut observers) = observers.get_mut(slot.entity) else {
            continue;
        };
        if !change.held {
            observers.0.retain(|player| *player != change.player);
        } else if !observers.0.contains(&change.player) {
            observers.0.push(change.player);
        }
    }
}

pub(crate) fn announce_held_columns(
    insert: On<Insert, ColumnView>,
    views: Query<(&ColumnView, &InDimension)>,
    mut held: MessageWriter<ColumnHeld>,
) {
    write_held(insert.event().entity, &views, &mut held, true);
}

pub(crate) fn withdraw_held_columns(
    discard: On<Discard, ColumnView>,
    views: Query<(&ColumnView, &InDimension)>,
    mut held: MessageWriter<ColumnHeld>,
) {
    write_held(discard.event().entity, &views, &mut held, false);
}

fn write_held(
    player: Entity,
    views: &Query<(&ColumnView, &InDimension)>,
    held: &mut MessageWriter<ColumnHeld>,
    holds: bool,
) {
    let Ok((view, in_dim)) = views.get(player) else {
        return;
    };
    held.write_batch(view.held().map(|column| ColumnHeld {
        player,
        dim: in_dim.0,
        column,
        held: holds,
    }));
}
