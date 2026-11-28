use crate::dimension_entry::DimensionEntry;
use crate::player_entry::PlayerEntry;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_resource::ResourceLocation;
use rustc_hash::FxHashMap;

#[derive(Resource, Debug, Default)]
pub struct CommandTargets {
    pub(crate) dimensions: Vec<DimensionEntry>,
    pub(crate) players: Vec<PlayerEntry>,
    pub(crate) by_session: FxHashMap<PlayerSession, usize>,
}

impl CommandTargets {
    pub fn dimensions(&self) -> &[DimensionEntry] {
        &self.dimensions
    }

    pub fn players(&self) -> &[PlayerEntry] {
        &self.players
    }

    pub fn dimension_of(&self, label: Entity) -> Option<usize> {
        self.dimensions
            .iter()
            .position(|entry| entry.label == label)
    }

    pub fn label_of(&self, index: usize) -> Option<Entity> {
        self.dimensions.get(index).map(|entry| entry.label)
    }

    pub fn dimension_named(&self, id: ResourceLocation<'_>) -> Option<&DimensionEntry> {
        self.dimensions
            .iter()
            .find(|entry| entry.key.namespace() == id.namespace() && entry.key.path() == id.path())
    }

    pub fn player(&self, session: PlayerSession) -> Option<&PlayerEntry> {
        self.players.get(*self.by_session.get(&session)?)
    }

    pub(crate) fn player_mut(&mut self, session: PlayerSession) -> Option<&mut PlayerEntry> {
        self.players.get_mut(*self.by_session.get(&session)?)
    }

    pub(crate) fn index_players(&mut self) {
        self.players.sort_unstable_by_key(|player| player.session.0);
        self.by_session.clear();
        self.by_session.extend(
            self.players
                .iter()
                .enumerate()
                .map(|(index, player)| (player.session, index)),
        );
    }
}
