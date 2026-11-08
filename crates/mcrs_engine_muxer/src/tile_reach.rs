use std::collections::BTreeMap;

use crate::planner::Point;
use crate::tile_key::TileKey;
use crate::view::View;

/// The largest tracking range that the muxer honours, in the units of the game. A larger range
/// from the codec counts as this one, so a bad value cannot make a region of millions of tiles.
pub(crate) const MAX_RANGE: u32 = 1536;

/// A held ranged subject stays with its viewer until this far past its range, so a subject that
/// wanders on the edge of the range does not spawn and despawn in each tick.
const KEEP_MARGIN: f64 = 1.0;

/// A viewer that moves less than this from where the muxer last measured its region keeps the
/// region, so a walking player does not judge the edge of its range again in each tick.
pub(crate) const RETRACK_STEP: f64 = 0.5;

/// The distances on X and Z from a viewer to the nearest and to the farthest point of a tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TileReach {
    pub(crate) near: f64,
    pub(crate) far: f64,
}

impl TileReach {
    pub(crate) fn new(point: Point, tile: TileKey, width: f64) -> Self {
        let axis = |value: f64, index: i32| {
            let start = f64::from(index) * width;
            let end = start + width;
            AxisReach {
                near: (start - value).max(value - end).max(0.0),
                far: (value - start).abs().max((end - value).abs()),
            }
        };
        let x = axis(point.x, tile.x);
        let z = axis(point.z, tile.z);
        Self {
            near: x.near.hypot(z.near),
            far: x.far.hypot(z.far),
        }
    }

    /// Whether each subject of the tile with a range of `range` or more pairs, whatever its
    /// position in the tile.
    pub(crate) fn is_inside(self, range: u32) -> bool {
        self.far <= f64::from(range)
    }

    /// Whether some subject of the tile with a range of at most `range` can pair or stay.
    pub(crate) fn can_reach(self, range: u32) -> bool {
        self.near <= f64::from(range) + KEEP_MARGIN
    }
}

struct AxisReach {
    near: f64,
    far: f64,
}

/// Whether a viewer keeps or gains a subject with `range` in a tile at `reach`. `held` says if the
/// client of the viewer holds the subject now. `distance` gives the distance on X and Z from the
/// viewer to the subject, and the muxer asks it only for a tile on the edge of the range.
pub(crate) fn tracks(
    reach: TileReach,
    range: u32,
    held: bool,
    distance: impl FnOnce() -> Option<f64>,
) -> bool {
    if reach.is_inside(range) {
        return true;
    }
    let limit = if held {
        f64::from(range) + KEEP_MARGIN
    } else {
        f64::from(range)
    };
    if reach.near > limit {
        return false;
    }
    distance().is_some_and(|distance| distance <= limit)
}

/// The tiles of the layers of `view` on Y whose nearest point on X and Z is within `range` and
/// its margin of `point`, each with its reach.
pub(crate) fn region(
    point: Point,
    view: &View,
    width: f64,
    range: u32,
) -> BTreeMap<TileKey, TileReach> {
    let mut tiles = BTreeMap::new();
    if range == 0 {
        return tiles;
    }
    let limit = f64::from(range.min(MAX_RANGE)) + KEEP_MARGIN;
    let span = |value: f64| {
        let first = ((value - limit) / width).floor() as i32;
        let last = ((value + limit) / width).floor() as i32;
        first..=last
    };
    let layers = i32::from(view.radius.y);
    let first_y = view.center.y.saturating_sub(layers);
    let last_y = view.center.y.saturating_add(layers);
    for x in span(point.x) {
        for z in span(point.z) {
            let reach = TileReach::new(point, TileKey::new(x, 0, z), width);
            if reach.near > limit {
                continue;
            }
            for y in first_y..=last_y {
                tiles.insert(TileKey::new(x, y, z), reach);
            }
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subject_on_the_edge_pairs_inside_its_range_and_leaves_only_past_the_margin() {
        let reach = TileReach {
            near: 150.0,
            far: 190.0,
        };
        assert!(tracks(reach, 160, false, || Some(160.0)));
        assert!(!tracks(reach, 160, false, || Some(160.5)));
        assert!(tracks(reach, 160, true, || Some(160.5)));
        assert!(!tracks(reach, 160, true, || Some(161.5)));
    }
}
