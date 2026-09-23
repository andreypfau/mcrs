use bevy_ecs::prelude::Component;
use mcrs_minecraft_core::{ColumnPos, SectionPos};

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerViewDistance {
    pub distance: u8,
    pub vert_distance: u8,
}

impl Default for PlayerViewDistance {
    fn default() -> Self {
        Self {
            distance: 12,
            vert_distance: 8,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct ChunkTrackingView {
    pub center: SectionPos,
    pub distance: u8,
    pub vert_distance: u8,
    /// Inclusive lower bound on section_y; iteration and `contains` will
    /// reject positions below this. `i32::MIN` disables the floor.
    pub min_section_y: i32,
    /// Inclusive upper bound on section_y; `i32::MAX` disables the ceiling.
    pub max_section_y: i32,
}

impl Default for ChunkTrackingView {
    fn default() -> Self {
        Self {
            center: SectionPos::new(0, 0, 0),
            distance: 12,
            vert_distance: 8,
            min_section_y: i32::MIN,
            max_section_y: i32::MAX,
        }
    }
}

impl ChunkTrackingView {
    pub fn new(center: SectionPos, distance: u8, vert_distance: u8) -> Self {
        Self::with_y_bounds(center, distance, vert_distance, i32::MIN, i32::MAX)
    }

    pub fn with_y_bounds(
        center: SectionPos,
        distance: u8,
        vert_distance: u8,
        min_section_y: i32,
        max_section_y: i32,
    ) -> Self {
        Self {
            center,
            distance,
            vert_distance,
            min_section_y,
            max_section_y,
        }
    }

    fn min_x(&self) -> i32 {
        self.center.x - (self.distance as i32 + 1)
    }
    fn min_z(&self) -> i32 {
        self.center.z - (self.distance as i32 + 1)
    }
    fn max_x(&self) -> i32 {
        self.center.x + (self.distance as i32 + 1)
    }
    fn max_z(&self) -> i32 {
        self.center.z + (self.distance as i32 + 1)
    }

    /// Whether any section of this column is still tracked. A column only
    /// truly leaves the view when its XZ does.
    pub fn contains_column(&self, x: i32, z: i32) -> bool {
        x.saturating_sub(self.center.x).unsigned_abs() <= self.distance as u32
            && z.saturating_sub(self.center.z).unsigned_abs() <= self.distance as u32
    }

    pub fn contains(&self, pos: &SectionPos) -> bool {
        // Saturating ops keep an extreme self.center.y from panicking in
        // debug builds.
        let dy = pos.y.saturating_sub(self.center.y).unsigned_abs();
        let dx = pos.x.saturating_sub(self.center.x).unsigned_abs();
        let dz = pos.z.saturating_sub(self.center.z).unsigned_abs();
        pos.y >= self.min_section_y
            && pos.y <= self.max_section_y
            && dy <= self.vert_distance as u32
            && dx <= self.distance as u32
            && dz <= self.distance as u32
    }

    /// Every column the view holds. The vertical extent is not part of it: a column is sent
    /// whole, so what the player is owed is decided in XZ alone.
    pub fn each_column(&self, mut f: impl FnMut(ColumnPos)) {
        let d = self.distance as i32;
        for x in self.center.x.saturating_sub(d)..=self.center.x.saturating_add(d) {
            for z in self.center.z.saturating_sub(d)..=self.center.z.saturating_add(d) {
                f(ColumnPos::new(x, z));
            }
        }
    }

    /// The columns the move added and dropped. A step that only changes the player's height
    /// moves no column either way, which is what keeps a vertical step from tearing down and
    /// re-sending the whole view.
    pub fn diff_columns(
        old: &ChunkTrackingView,
        new: &ChunkTrackingView,
        mut on_load: impl FnMut(ColumnPos),
        mut on_unload: impl FnMut(ColumnPos),
    ) {
        if old.center.x == new.center.x
            && old.center.z == new.center.z
            && old.distance == new.distance
        {
            return;
        }
        let min_x = old.min_x().min(new.min_x());
        let max_x = old.max_x().max(new.max_x());
        let min_z = old.min_z().min(new.min_z());
        let max_z = old.max_z().max(new.max_z());
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let was = old.contains_column(x, z);
                let is = new.contains_column(x, z);
                match (was, is) {
                    (false, true) => on_load(ColumnPos::new(x, z)),
                    (true, false) => on_unload(ColumnPos::new(x, z)),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod contains_column_tests {
    use super::*;

    #[test]
    fn a_vertical_step_keeps_every_column_the_view_already_held() {
        let view = ChunkTrackingView::new(SectionPos::new(0, 4, 0), 12, 8);
        let stepped = ChunkTrackingView::new(SectionPos::new(0, 5, 0), 12, 8);

        // The section that drops out of the bottom of the vertical window.
        let evicted = SectionPos::new(3, -4, 7);
        assert!(view.contains(&evicted));
        assert!(!stepped.contains(&evicted));

        // Its column is still tracked, so the column must not be torn down.
        assert!(stepped.contains_column(evicted.x, evicted.z));
    }

    #[test]
    fn a_column_outside_the_horizontal_reach_is_gone() {
        let view = ChunkTrackingView::new(SectionPos::new(0, 4, 0), 12, 8);
        assert!(view.contains_column(12, -12));
        assert!(!view.contains_column(13, 0));
        assert!(!view.contains_column(0, -13));
    }
}
