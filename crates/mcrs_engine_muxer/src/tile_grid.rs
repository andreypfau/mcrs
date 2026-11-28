use std::num::NonZeroU32;

use mcrs_minecraft_entity_key::TileKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileGrid {
    pub x: NonZeroU32,
    pub y: NonZeroU32,
    pub z: NonZeroU32,
}

impl TileGrid {
    #[must_use]
    pub fn tile_at(self, x: i32, y: i32, z: i32) -> TileKey {
        TileKey::new(
            floor_div(x, self.x),
            floor_div(y, self.y),
            floor_div(z, self.z),
        )
    }
}

fn floor_div(value: i32, size: NonZeroU32) -> i32 {
    let quotient = i64::from(value).div_euclid(i64::from(size.get()));
    i32::try_from(quotient).unwrap_or_default()
}
