use std::collections::BTreeMap;

use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::session::PlayerSession;

use crate::player_view::PlayerView;

#[derive(Resource, Debug, Default)]
pub struct ForeignViews(BTreeMap<u64, ChunkTrackingView>);

impl ForeignViews {
    pub fn apply(&mut self, view: PlayerView) -> bool {
        match view.view {
            Some(tracked) => self.0.insert(view.session.0, tracked) != Some(tracked),
            None => self.0.remove(&view.session.0).is_some(),
        }
    }

    pub(crate) fn forget(&mut self, session: PlayerSession) {
        self.0.remove(&session.0);
    }

    pub fn views(&self) -> impl Iterator<Item = ChunkTrackingView> + '_ {
        self.0.values().copied()
    }
}
