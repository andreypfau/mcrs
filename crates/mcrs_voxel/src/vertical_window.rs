use crate::{BlockPos, SectionPos};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct VerticalWindow {
    first_section: i32,
    num_sections: u32,
}

impl Default for VerticalWindow {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl VerticalWindow {
    pub const IDENTITY: Self = Self {
        first_section: 0,
        num_sections: 0,
    };

    #[inline]
    #[must_use]
    pub const fn new(first_section: i32, num_sections: u32) -> Self {
        Self {
            first_section,
            num_sections,
        }
    }

    #[inline]
    #[must_use]
    pub const fn is_identity(&self) -> bool {
        self.num_sections == 0
    }

    #[inline]
    #[must_use]
    pub const fn first_section(&self) -> i32 {
        self.first_section
    }

    #[inline]
    #[must_use]
    pub const fn num_sections(&self) -> u32 {
        self.num_sections
    }

    #[inline]
    #[must_use]
    pub const fn min_block_y(&self) -> i32 {
        if self.is_identity() {
            i32::MIN
        } else {
            self.first_section.saturating_mul(16)
        }
    }

    #[inline]
    #[must_use]
    pub const fn max_block_y(&self) -> i32 {
        if self.is_identity() {
            i32::MAX
        } else {
            let span = match (self.num_sections as i64).checked_mul(16) {
                Some(s) => s,
                None => i64::MAX,
            };
            let min_y = (self.first_section as i64).saturating_mul(16);
            let max_y = min_y.saturating_add(span).saturating_sub(1);
            if max_y > i32::MAX as i64 {
                i32::MAX
            } else if max_y < i32::MIN as i64 {
                i32::MIN
            } else {
                max_y as i32
            }
        }
    }

    #[inline]
    #[must_use]
    pub const fn contains_section(&self, section_y: i32) -> bool {
        if self.is_identity() {
            return true;
        }
        let diff = (section_y as i64) - (self.first_section as i64);
        diff >= 0 && diff < (self.num_sections as i64)
    }

    #[inline]
    #[must_use]
    pub const fn contains_block_y(&self, block_y: i32) -> bool {
        if self.is_identity() {
            return true;
        }
        let min_y = (self.first_section as i64).saturating_mul(16);
        let span = match (self.num_sections as i64).checked_mul(16) {
            Some(s) => s,
            None => i64::MAX,
        };
        let b = block_y as i64;
        b >= min_y && b < min_y.saturating_add(span)
    }

    #[inline]
    #[must_use]
    pub const fn to_window_section_y(&self, section_y: i32) -> Option<i32> {
        if self.is_identity() {
            Some(section_y)
        } else {
            let diff = (section_y as i64) - (self.first_section as i64);
            if diff >= 0 && diff < (self.num_sections as i64) && diff <= (i32::MAX as i64) {
                Some(diff as i32)
            } else {
                None
            }
        }
    }

    #[inline]
    #[must_use]
    pub const fn from_window_section_y(&self, window_section_y: i32) -> i32 {
        if self.is_identity() {
            window_section_y
        } else {
            self.first_section.saturating_add(window_section_y)
        }
    }

    #[inline]
    #[must_use]
    pub const fn to_window_block_y(&self, block_y: i32) -> Option<i32> {
        if self.is_identity() {
            Some(block_y)
        } else if self.contains_block_y(block_y) {
            let offset = (self.first_section as i64).saturating_mul(16);
            let rel = (block_y as i64).saturating_sub(offset);
            if rel >= (i32::MIN as i64) && rel <= (i32::MAX as i64) {
                Some(rel as i32)
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub const fn from_window_block_y(&self, window_block_y: i32) -> i32 {
        if self.is_identity() {
            window_block_y
        } else {
            let offset = (self.first_section as i64).saturating_mul(16);
            let canon = (window_block_y as i64).saturating_add(offset);
            if canon > i32::MAX as i64 {
                i32::MAX
            } else if canon < i32::MIN as i64 {
                i32::MIN
            } else {
                canon as i32
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn translate_block_pos(&self, pos: BlockPos) -> Option<BlockPos> {
        let y = self.to_window_block_y(pos.y)?;
        Some(BlockPos::new(pos.x, y, pos.z))
    }

    #[inline]
    #[must_use]
    pub fn untranslate_block_pos(&self, pos: BlockPos) -> BlockPos {
        let y = self.from_window_block_y(pos.y);
        BlockPos::new(pos.x, y, pos.z)
    }

    #[inline]
    #[must_use]
    pub fn translate_section_pos(&self, pos: SectionPos) -> Option<SectionPos> {
        let y = self.to_window_section_y(pos.y)?;
        Some(SectionPos::new(pos.x, y, pos.z))
    }

    #[inline]
    #[must_use]
    pub fn untranslate_section_pos(&self, pos: SectionPos) -> SectionPos {
        let y = self.from_window_section_y(pos.y);
        SectionPos::new(pos.x, y, pos.z)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct VerticalGridPolicy {
    pub grid_step: u32,
    pub margin_sections: u32,
}

impl Default for VerticalGridPolicy {
    #[inline]
    fn default() -> Self {
        Self {
            grid_step: 8,
            margin_sections: 4,
        }
    }
}

impl VerticalGridPolicy {
    #[inline]
    #[must_use]
    pub const fn new(grid_step: u32, margin_sections: u32) -> Self {
        Self {
            grid_step: if grid_step == 0 { 1 } else { grid_step },
            margin_sections,
        }
    }

    #[must_use]
    pub fn select_window(
        &self,
        player_section_y: i32,
        window_height_sections: u32,
    ) -> VerticalWindow {
        if window_height_sections == 0 {
            return VerticalWindow::IDENTITY;
        }

        let half_height = (window_height_sections / 2) as i32;
        let ideal_first = player_section_y.saturating_sub(half_height);

        let step = if self.grid_step == 0 {
            1
        } else {
            self.grid_step as i64
        };
        let rem = (ideal_first as i64).rem_euclid(step);
        let aligned_first = (ideal_first as i64) - rem;

        let clamped_first = if aligned_first > i32::MAX as i64 {
            i32::MAX
        } else if aligned_first < i32::MIN as i64 {
            i32::MIN
        } else {
            aligned_first as i32
        };

        VerticalWindow::new(clamped_first, window_height_sections)
    }

    #[must_use]
    pub fn needs_rebase(&self, current_window: &VerticalWindow, player_section_y: i32) -> bool {
        if current_window.is_identity() {
            return false;
        }

        let first = current_window.first_section() as i64;
        let last = first + (current_window.num_sections() as i64) - 1;
        let margin = self.margin_sections as i64;
        let p = player_section_y as i64;

        p < first + margin || p > last - margin
    }
}
