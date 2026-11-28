use crate::tile_reach;
use crate::tile_reach::TileReach;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Place {
    reach: TileReach,
    range: u32,
}

impl Place {
    pub(crate) fn new(reach: TileReach, range: u32) -> Self {
        Self { reach, range }
    }

    pub(crate) fn tracks(self, held: bool, distance: impl FnOnce() -> Option<f64>) -> bool {
        tile_reach::tracks(self.reach, self.range, held, distance)
    }
}
