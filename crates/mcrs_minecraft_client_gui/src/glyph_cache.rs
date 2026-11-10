use std::collections::HashMap;

use bevy::prelude::Resource;

use crate::font_unihex::CELL;

/// Cells of the atlas region the Unifont glyphs on screen take turns in.
pub const COLUMNS: usize = 32;
pub const ROWS: usize = 32;
pub const SLOTS: usize = COLUMNS * ROWS;

/// Which Unifont glyph holds each cell of the atlas's glyph region. The batch carries this table
/// to the render world, which draws a glyph into every cell whose content changed there; so a frame
/// the render world missed costs nothing.
#[derive(Resource, Debug)]
pub struct GlyphCache {
    frame: u64,
    resident: Vec<Option<char>>,
    used: Vec<u64>,
    cells: HashMap<char, usize>,
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self {
            frame: 0,
            resident: vec![None; SLOTS],
            used: vec![0; SLOTS],
            cells: HashMap::new(),
        }
    }
}

impl GlyphCache {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
    }

    pub fn resident(&self) -> &[Option<char>] {
        &self.resident
    }

    /// The cell holding `ch`, which a miss fills by evicting the cell unused for longest. `None`
    /// when every cell is in use this frame.
    // chisle: a frame that shows more than `SLOTS` distinct Unifont glyphs leaves the rest
    // undrawn; a larger region lifts it.
    pub fn cell(&mut self, ch: char) -> Option<usize> {
        if let Some(&cell) = self.cells.get(&ch) {
            if let Some(used) = self.used.get_mut(cell) {
                *used = self.frame;
            }
            return Some(cell);
        }
        let victim = self.resident.iter().position(Option::is_none).or_else(|| {
            (0..SLOTS)
                .filter(|&cell| self.used.get(cell).is_some_and(|&used| used < self.frame))
                .min_by_key(|&cell| self.used.get(cell).copied())
        })?;
        if let Some(old) = self.resident.get(victim).copied().flatten() {
            self.cells.remove(&old);
        }
        if let (Some(resident), Some(used)) =
            (self.resident.get_mut(victim), self.used.get_mut(victim))
        {
            *resident = Some(ch);
            *used = self.frame;
        }
        self.cells.insert(ch, victim);
        Some(victim)
    }
}

/// The top left texel of `cell` in the glyph region.
pub fn cell_origin(cell: usize) -> (u32, u32) {
    (
        (cell % COLUMNS * CELL) as u32,
        (cell / COLUMNS * CELL) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_keeps_its_cell_and_a_full_cache_evicts_what_this_frame_did_not_use() {
        let mut cache = GlyphCache::default();
        cache.begin_frame();
        let cells: Vec<_> = (0..SLOTS as u32)
            .map(|i| cache.cell(char::from_u32(0x4E00 + i).unwrap()).unwrap())
            .collect();
        assert_eq!(cells, (0..SLOTS).collect::<Vec<_>>());
        assert_eq!(cache.cell('\u{4E00}'), Some(0));
        assert_eq!(
            cache.cell('\u{3042}'),
            None,
            "every cell is in use this frame"
        );

        cache.begin_frame();
        assert_eq!(cache.cell('\u{4E01}'), Some(1));
        let fresh = cache.cell('\u{3042}').unwrap();
        assert_ne!(fresh, 1, "a glyph used this frame is not evicted");
        assert_eq!(cache.resident()[fresh], Some('\u{3042}'));
        assert_eq!(cell_origin(COLUMNS + 1), (16, 16));
    }
}
