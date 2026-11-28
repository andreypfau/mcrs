use std::collections::BTreeMap;

use crate::planner::Point;
use crate::view::View;
use mcrs_minecraft_entity_key::TileKey;

pub(crate) const MAX_RANGE: u32 = 1536;

const KEEP_MARGIN: f64 = 1.0;

pub(crate) const RETRACK_STEP: f64 = 0.5;

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

    pub(crate) fn is_inside(self, range: u32) -> bool {
        self.far <= f64::from(range)
    }

    pub(crate) fn can_reach(self, range: u32) -> bool {
        self.near <= f64::from(range) + KEEP_MARGIN
    }
}

struct AxisReach {
    near: f64,
    far: f64,
}

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
        if self.reach.is_inside(self.range) {
            return true;
        }
        let limit = if held {
            f64::from(self.range) + KEEP_MARGIN
        } else {
            f64::from(self.range)
        };
        if self.reach.near > limit {
            return false;
        }
        distance().is_some_and(|distance| distance <= limit)
    }
}

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
        assert!(Place::new(reach, 160).tracks(false, || Some(160.0)));
        assert!(!Place::new(reach, 160).tracks(false, || Some(160.5)));
        assert!(Place::new(reach, 160).tracks(true, || Some(160.5)));
        assert!(!Place::new(reach, 160).tracks(true, || Some(161.5)));
    }
}
