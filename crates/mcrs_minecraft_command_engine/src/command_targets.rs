use crate::dimension_entry::DimensionEntry;
use crate::player_entry::PlayerEntry;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_resource::ResourceLocation;
use rustc_hash::FxHashMap;

/// The dimensions in the dimension order and the online players of all of them in
/// the order of the player list. A world collects it before it runs the commands of a tick: its
/// own players as they are, and those of the other worlds as they were at the end of the last
/// tick. The [`Worlds`](crate::Worlds) operations that change a player update its entry.
#[derive(Resource, Debug, Default)]
pub struct CommandTargets {
    pub(crate) dimensions: Vec<DimensionEntry>,
    pub(crate) players: Vec<PlayerEntry>,
    pub(crate) by_session: FxHashMap<PlayerSession, usize>,
}

impl CommandTargets {
    /// The dimensions that run, in the dimension order.
    pub fn dimensions(&self) -> &[DimensionEntry] {
        &self.dimensions
    }

    /// The online players of every dimension, in the order of the player list.
    pub fn players(&self) -> &[PlayerEntry] {
        &self.players
    }

    /// The index in the dimension order of the dimension labelled `label`.
    pub fn dimension_of(&self, label: Entity) -> Option<usize> {
        self.dimensions
            .iter()
            .position(|entry| entry.label == label)
    }

    /// The label of the dimension at `index` in the dimension order.
    pub fn label_of(&self, index: usize) -> Option<Entity> {
        self.dimensions.get(index).map(|entry| entry.label)
    }

    /// The running dimension with the identifier `id`.
    pub fn dimension_named(&self, id: ResourceLocation<'_>) -> Option<&DimensionEntry> {
        self.dimensions
            .iter()
            .find(|entry| entry.key.namespace() == id.namespace() && entry.key.path() == id.path())
    }

    /// The online player of `session`.
    pub fn player(&self, session: PlayerSession) -> Option<&PlayerEntry> {
        self.players.get(*self.by_session.get(&session)?)
    }

    pub(crate) fn player_mut(&mut self, session: PlayerSession) -> Option<&mut PlayerEntry> {
        self.players.get_mut(*self.by_session.get(&session)?)
    }

    pub(crate) fn index_players(&mut self) {
        self.players
            .sort_unstable_by_key(|player| player.list_order);
        self.by_session.clear();
        self.by_session.extend(
            self.players
                .iter()
                .enumerate()
                .map(|(index, player)| (player.session, index)),
        );
    }
}
