use bevy_ecs::resource::Resource;
use mcrs_minecraft_entity_key::TileKey;

#[derive(Resource, Debug, Default)]
pub struct StirredTiles(Vec<TileKey>);

impl StirredTiles {
    pub fn replace(&mut self, tiles: impl IntoIterator<Item = TileKey>) {
        let stirred = &mut self.0;
        stirred.clear();
        stirred.extend(tiles);
        stirred.sort_unstable();
        stirred.dedup();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn wakes(&self, tile: TileKey) -> bool {
        if self.0.is_empty() {
            return false;
        }
        (-1..=1).any(|z| {
            (-1..=1).any(|x| {
                let near = TileKey::new(tile.x.saturating_add(x), 0, tile.z.saturating_add(z));
                self.0.binary_search(&near).is_ok()
            })
        })
    }
}
