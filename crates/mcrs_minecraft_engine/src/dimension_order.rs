use bevy_ecs::world::World;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::keys::dimension::{OVERWORLD, THE_END, THE_NETHER};
use mcrs_minecraft_resource::ResourceKeyBuf;

/// The rank of a dimension in the dimension order: the three vanilla dimensions first, then
/// the others by identifier.
fn dimension_rank(key: &ResourceKeyBuf<Dimension>) -> (u8, &str) {
    let rank = [OVERWORLD, THE_NETHER, THE_END]
        .iter()
        .position(|vanilla| vanilla.as_str() == key.as_str())
        .unwrap_or(3) as u8;
    (rank, key.as_str())
}

/// A dimension world and what [`in_dimension_order`] read from it.
pub struct DimensionWorld<'w, T> {
    /// What the reader took from the world.
    pub read: T,
    /// The world of the dimension.
    pub world: &'w mut World,
}

/// The worlds that `read` accepts, each with what it read, in the dimension order of `key`.
pub fn in_dimension_order<'w, T>(
    worlds: impl Iterator<Item = &'w mut World>,
    mut read: impl FnMut(&mut World) -> Option<T>,
    key: impl Fn(&T) -> &ResourceKeyBuf<Dimension>,
    found: &mut Vec<DimensionWorld<'w, T>>,
) {
    found.clear();
    found.extend(worlds.filter_map(|world| {
        Some(DimensionWorld {
            read: read(world)?,
            world,
        })
    }));
    found.sort_by(|a, b| dimension_rank(key(&a.read)).cmp(&dimension_rank(key(&b.read))));
}
