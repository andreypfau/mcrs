use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::TileKey;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublishedTile(pub(crate) Option<TileKey>);

impl PublishedTile {
    #[must_use]
    pub fn tile(self) -> Option<TileKey> {
        self.0
    }
}
