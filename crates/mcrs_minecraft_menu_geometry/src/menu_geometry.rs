use mcrs_minecraft_client_widget::{GuiPoint, GuiRect};
use mcrs_minecraft_container_click::Clicked;

use crate::EntryCorner;
use crate::inventory_entries::INVENTORY_ENTRIES;

const ENTRY_SIZE: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuGeometry {
    width: i32,
    height: i32,
    entries: &'static [GuiPoint],
}

impl MenuGeometry {
    pub const INVENTORY: Self = Self {
        width: 176,
        height: 166,
        entries: &INVENTORY_ENTRIES,
    };

    #[must_use]
    pub const fn width(&self) -> i32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    #[must_use]
    pub const fn window(&self, screen_width: i32, screen_height: i32) -> GuiPoint {
        GuiPoint::new(
            screen_width.saturating_sub(self.width).div_euclid(2),
            screen_height.saturating_sub(self.height).div_euclid(2),
        )
    }

    #[must_use]
    pub fn entry(&self, index: u16) -> Option<GuiPoint> {
        self.entries.get(usize::from(index)).copied()
    }

    pub fn entries(&self) -> impl Iterator<Item = EntryCorner> + use<> {
        (0..=u16::MAX)
            .zip(self.entries)
            .map(|(index, &corner)| EntryCorner { index, corner })
    }

    #[must_use]
    pub fn clicked_at(&self, window: GuiPoint, pointer: GuiPoint) -> Clicked {
        let local = GuiPoint::new(
            pointer.x.saturating_sub(window.x),
            pointer.y.saturating_sub(window.y),
        );
        if !(0..self.width).contains(&local.x) || !(0..self.height).contains(&local.y) {
            return Clicked::Outside;
        }
        self.entries()
            .find(|entry| {
                GuiRect::new(entry.corner.x, entry.corner.y, ENTRY_SIZE, ENTRY_SIZE)
                    .contains_point(local)
            })
            .map_or(Clicked::Nowhere, |entry| Clicked::Entry(entry.index))
    }
}
