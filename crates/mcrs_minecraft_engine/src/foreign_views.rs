use std::collections::BTreeMap;

use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::session::PlayerSession;

use crate::player_view::PlayerView;

/// The views of players by session, in a fixed order. In a cell, the views of the players of the
/// other cells of the world: the muxer asks the owner of a column for it, whoever views it, so
/// the stages of cold space of a cell serve these views as well as the views of its own players.
#[derive(Resource, Debug, Default)]
pub struct ForeignViews(BTreeMap<u64, ChunkTrackingView>);

impl ForeignViews {
    /// Takes in a view that a cell reported, and tells whether the views changed.
    pub fn apply(&mut self, view: PlayerView) -> bool {
        match view.view {
            Some(tracked) => self.0.insert(view.session.0, tracked) != Some(tracked),
            None => self.0.remove(&view.session.0).is_some(),
        }
    }

    /// Forgets the view of a player that this cell holds from now on.
    pub(crate) fn forget(&mut self, session: PlayerSession) {
        self.0.remove(&session.0);
    }

    pub fn views(&self) -> impl Iterator<Item = ChunkTrackingView> + '_ {
        self.0.values().copied()
    }
}
