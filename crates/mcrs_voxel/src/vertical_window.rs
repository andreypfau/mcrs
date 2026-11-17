//! Vertical window mapping a tall or unbounded dimension onto the client height range.
//!
//! Defined per Issue #677 and Doctrine #566:
//! - Pure `#![no_std]` data model without Bevy and without panics.
//! - Bounded memory footprint (at most 16 bytes).
//! - Bi-directional panic-free coordinate translation between canonical and window-relative coordinates.
//! - Grid-aligned window selection so players at the same height share the identical window.

use crate::{BlockPos, SectionPos};

/// A vertical window mapping canonical dimension heights into a client's visible range.
///
/// A connection has a vertical window: the first canonical section that the client sees
/// as its own section 0, and the count of visible sections.
///
/// When `num_sections == 0`, the window is the **identity window**: the dimension fits
/// the client's native range, and coordinates pass through untranslated.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct VerticalWindow {
    /// The first canonical section that the client sees as its section 0.
    first_section: i32,
    /// The number of sections visible in this window (0 = identity / unbounded).
    num_sections: u32,
}

impl Default for VerticalWindow {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl VerticalWindow {
    /// The identity window for dimensions that fit the client's native height range.
    /// In an identity window, coordinates are never translated.
    pub const IDENTITY: Self = Self {
        first_section: 0,
        num_sections: 0,
    };

    /// Creates a new vertical window.
    ///
    /// - `first_section`: canonical section Y mapped to client section 0.
    /// - `num_sections`: number of sections visible (passing 0 creates an identity window).
    #[inline]
    #[must_use]
    pub const fn new(first_section: i32, num_sections: u32) -> Self {
        Self {
            first_section,
            num_sections,
        }
    }

    /// Returns `true` if this is the identity window.
    #[inline]
    #[must_use]
    pub const fn is_identity(&self) -> bool {
        self.num_sections == 0
    }

    /// The first canonical section that the client sees as its section 0.
    #[inline]
    #[must_use]
    pub const fn first_section(&self) -> i32 {
        self.first_section
    }

    /// The number of sections visible in this window (0 if identity).
    #[inline]
    #[must_use]
    pub const fn num_sections(&self) -> u32 {
        self.num_sections
    }

    /// The lowest canonical block Y included in this window.
    #[inline]
    #[must_use]
    pub const fn min_block_y(&self) -> i32 {
        if self.is_identity() {
            i32::MIN
        } else {
            self.first_section.saturating_mul(16)
        }
    }

    /// The highest canonical block Y included in this window.
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

    /// Returns `true` if the given canonical section Y is inside this window.
    #[inline]
    #[must_use]
    pub const fn contains_section(&self, section_y: i32) -> bool {
        if self.is_identity() {
            return true;
        }
        let diff = (section_y as i64) - (self.first_section as i64);
        diff >= 0 && diff < (self.num_sections as i64)
    }

    /// Returns `true` if the given canonical block Y is inside this window.
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

    /// Translates a canonical section Y to a window-relative section index.
    ///
    /// Returns `Some(window_section)` if within the window, or `None` if out of bounds.
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

    /// Untranslates a window-relative section index back to a canonical section Y.
    #[inline]
    #[must_use]
    pub const fn from_window_section_y(&self, window_section_y: i32) -> i32 {
        if self.is_identity() {
            window_section_y
        } else {
            self.first_section.saturating_add(window_section_y)
        }
    }

    /// Translates a canonical block Y to a window-relative block Y.
    ///
    /// Returns `Some(window_y)` if within the window, or `None` if out of bounds.
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

    /// Untranslates a window-relative block Y back to canonical block Y.
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

    /// Translates an outbound `BlockPos` from canonical to window coordinates.
    #[inline]
    #[must_use]
    pub fn translate_block_pos(&self, pos: BlockPos) -> Option<BlockPos> {
        let y = self.to_window_block_y(pos.y)?;
        Some(BlockPos::new(pos.x, y, pos.z))
    }

    /// Untranslates an inbound `BlockPos` from window coordinates back to canonical coordinates.
    #[inline]
    #[must_use]
    pub fn untranslate_block_pos(&self, pos: BlockPos) -> BlockPos {
        let y = self.from_window_block_y(pos.y);
        BlockPos::new(pos.x, y, pos.z)
    }

    /// Translates an outbound `SectionPos` from canonical to window coordinates.
    #[inline]
    #[must_use]
    pub fn translate_section_pos(&self, pos: SectionPos) -> Option<SectionPos> {
        let y = self.to_window_section_y(pos.y)?;
        Some(SectionPos::new(pos.x, y, pos.z))
    }

    /// Untranslates an inbound `SectionPos` from window coordinates back to canonical coordinates.
    #[inline]
    #[must_use]
    pub fn untranslate_section_pos(&self, pos: SectionPos) -> SectionPos {
        let y = self.from_window_section_y(pos.y);
        SectionPos::new(pos.x, y, pos.z)
    }
}

/// Global grid policy for allocating vertical windows.
///
/// Ensures two players at the same height get the same window, minimizing duplicate
/// broadcasts, and applies hysteresis margins so players near boundaries do not rapidly thrash windows.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct VerticalGridPolicy {
    /// Number of sections per grid step (e.g. 8 or 16).
    pub grid_step: u32,
    /// Number of sections near boundary before triggering a rebase.
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
    /// Creates a new vertical grid policy.
    #[inline]
    #[must_use]
    pub const fn new(grid_step: u32, margin_sections: u32) -> Self {
        Self {
            grid_step: if grid_step == 0 { 1 } else { grid_step },
            margin_sections,
        }
    }

    /// Calculates the canonical window for a given player section Y and window height.
    ///
    /// Snaps the center of the window to the nearest grid step.
    #[must_use]
    pub fn select_window(&self, player_section_y: i32, window_height_sections: u32) -> VerticalWindow {
        if window_height_sections == 0 {
            return VerticalWindow::IDENTITY;
        }

        let half_height = (window_height_sections / 2) as i32;
        let ideal_first = player_section_y.saturating_sub(half_height);

        let step = if self.grid_step == 0 { 1 } else { self.grid_step as i64 };
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

    /// Checks if a player's position has drifted close enough to the window edge
    /// that a rebase should be scheduled.
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
