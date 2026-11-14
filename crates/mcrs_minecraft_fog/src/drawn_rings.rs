use alloc::vec::Vec;

const BLOCKS_PER_CHUNK: f32 = 16.0;

/// For one render distance, the count of drawn columns in each ring around the column of the
/// camera against the count of columns in that ring that pass the draw test. A column is given
/// by its offset in columns from the column of the camera; each column is inserted at most once.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawnRings {
    render_distance: u8,
    drawn: Vec<u32>,
    expected: Vec<u32>,
}

impl DrawnRings {
    /// The rings of `render_distance` chunks, with no column drawn.
    #[must_use]
    pub fn new(render_distance: u8) -> Self {
        let mut rings = Self::default();
        rings.set_render_distance(render_distance);
        rings
    }

    /// The render distance in chunks.
    #[must_use]
    pub fn render_distance(&self) -> u8 {
        self.render_distance
    }

    /// Sets the render distance in chunks and forgets every drawn column.
    pub fn set_render_distance(&mut self, render_distance: u8) {
        self.render_distance = render_distance;
        let rings = usize::from(render_distance);
        self.drawn.clear();
        self.drawn.resize(rings, 0);
        self.expected.clear();
        self.expected.resize(rings, 0);
        let reach = i32::from(render_distance);
        for dx in -reach..=reach {
            for dz in -reach..=reach {
                if let Some(count) =
                    ring(render_distance, dx, dz).and_then(|ring| self.expected.get_mut(ring))
                {
                    *count = count.saturating_add(1);
                }
            }
        }
    }

    /// Forgets every drawn column.
    pub fn clear(&mut self) {
        self.drawn.fill(0);
    }

    /// Counts the column at (`dx`, `dz`) as drawn. A column that fails the draw test is ignored.
    pub fn insert(&mut self, dx: i32, dz: i32) {
        if let Some(count) = self.drawn_count_mut(dx, dz) {
            *count = count.saturating_add(1);
        }
    }

    /// Counts the column at (`dx`, `dz`) as no longer drawn. A column that fails the draw test
    /// is ignored.
    pub fn remove(&mut self, dx: i32, dz: i32) {
        if let Some(count) = self.drawn_count_mut(dx, dz) {
            *count = count.saturating_sub(1);
        }
    }

    /// The distance in blocks that the end of the render-distance fog moves toward: 16 times the
    /// smallest ring with a column that passes the draw test and is not drawn, or 16 times the
    /// render distance when each such column is drawn.
    #[must_use]
    pub fn fog_end_target(&self) -> f32 {
        let complete_rings = self
            .drawn
            .iter()
            .zip(&self.expected)
            .take_while(|(drawn, expected)| drawn >= expected)
            .count();
        BLOCKS_PER_CHUNK * complete_rings as f32
    }

    fn drawn_count_mut(&mut self, dx: i32, dz: i32) -> Option<&mut u32> {
        ring(self.render_distance, dx, dz).and_then(|ring| self.drawn.get_mut(ring))
    }
}

fn ring(render_distance: u8, dx: i32, dz: i32) -> Option<usize> {
    let gap = |offset: i32| u64::from(offset.unsigned_abs().saturating_sub(1));
    let squared = gap(dx) * gap(dx) + gap(dz) * gap(dz);
    let reach = u64::from(render_distance);
    if squared >= reach * reach {
        return None;
    }
    usize::try_from(squared.isqrt()).ok()
}
