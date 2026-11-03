use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Component;
use mcrs_minecraft_protocol_game::ColumnPos;
use rustc_hash::FxHashMap;

/// The players of a dimension whose `ColumnView` waits on each column, so that a column that
/// moves on reaches only the players it concerns. A projection of the views: only
/// `ColumnView::set` and the view's discard write it.
#[derive(Component, Default)]
pub(crate) struct ColumnWaiters {
    players: FxHashMap<ColumnPos, Vec<Entity>>,
    /// The lists of columns no player waits on any more, kept so the next column reuses one.
    spare: Vec<Vec<Entity>>,
}

impl ColumnWaiters {
    pub(crate) fn add(&mut self, column: ColumnPos, player: Entity) {
        let spare = &mut self.spare;
        self.players
            .entry(column)
            .or_insert_with(|| spare.pop().unwrap_or_default())
            .push(player);
    }

    pub(crate) fn remove(&mut self, column: ColumnPos, player: Entity) {
        let Some(players) = self.players.get_mut(&column) else {
            return;
        };
        players.retain(|waiting| *waiting != player);
        if players.is_empty()
            && let Some(empty) = self.players.remove(&column)
        {
            self.spare.push(empty);
        }
    }

    pub(crate) fn of(&self, column: ColumnPos) -> &[Entity] {
        self.players.get(&column).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn columns(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.players.keys().copied()
    }
}
