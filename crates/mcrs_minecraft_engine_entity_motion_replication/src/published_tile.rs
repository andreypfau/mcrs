use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::TileKey;

/// The tile in which the viewers know a moving entity: the tile of its spawn, then of its last
/// `Enter`. [`MotionPublish`](crate::MotionPublish) writes it; it is `None` until the spawn is
/// published. An event of the entity that another plugin publishes goes to this tile.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublishedTile(pub(crate) Option<TileKey>);

impl PublishedTile {
    #[must_use]
    pub fn tile(self) -> Option<TileKey> {
        self.0
    }
}
