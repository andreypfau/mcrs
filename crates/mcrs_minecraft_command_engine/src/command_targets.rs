use crate::dimension_entry::DimensionEntry;
use crate::player_entry::PlayerEntry;
use bevy_ecs::entity::{Entity, EntityHashMap};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_command::Identifier;

/// The dimensions in the dimension order and the online players of all of them in
/// the order of the player list. The host collects it at the start of a pass, and the
/// [`Worlds`](crate::Worlds) operations that change a player update its entry; a
/// handler reads it in the world it runs in.
#[derive(Resource, Debug, Default)]
pub struct CommandTargets {
    pub(crate) dimensions: Vec<DimensionEntry>,
    pub(crate) players: Vec<PlayerEntry>,
    pub(crate) by_anchor: EntityHashMap<usize>,
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
    pub fn dimension_named(&self, id: Identifier<'_>) -> Option<&DimensionEntry> {
        self.dimensions
            .iter()
            .find(|entry| entry.key.namespace() == id.namespace && entry.key.path() == id.path)
    }

    /// The online player with the host anchor `anchor`.
    pub fn player(&self, anchor: Entity) -> Option<&PlayerEntry> {
        self.players.get(*self.by_anchor.get(&anchor)?)
    }

    pub(crate) fn player_mut(&mut self, anchor: Entity) -> Option<&mut PlayerEntry> {
        self.players.get_mut(*self.by_anchor.get(&anchor)?)
    }

    pub(crate) fn index_players(&mut self) {
        self.players
            .sort_unstable_by_key(|player| player.list_order);
        self.by_anchor.clear();
        self.by_anchor.extend(
            self.players
                .iter()
                .enumerate()
                .map(|(index, player)| (player.anchor, index)),
        );
    }
}
