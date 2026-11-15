use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_entity_key::TileKey;

/// The tiles in which a block changed since the last pass, sorted and without duplicates.
#[derive(Resource, Debug, Default)]
pub(crate) struct StirredTiles(Vec<TileKey>);

impl StirredTiles {
    /// True when a block changed in `tile` or in one of the 8 tiles around it.
    pub(crate) fn wakes(&self, tile: TileKey) -> bool {
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

pub(crate) fn stir_tiles(
    mut placed: MessageReader<BlockPlaced>,
    mut stirred: ResMut<StirredTiles>,
) {
    let tiles = &mut stirred.0;
    tiles.clear();
    tiles.extend(
        placed
            .read()
            .map(|block| TileKey::at(block.block_pos.as_dvec3())),
    );
    tiles.sort_unstable();
    tiles.dedup();
}
