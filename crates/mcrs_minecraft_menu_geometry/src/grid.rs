use mcrs_minecraft_client_widget::GuiPoint;

pub(crate) const SLOT_PITCH: i32 = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Grid {
    pub(crate) origin: GuiPoint,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
}

impl Grid {
    pub(crate) const fn len(self) -> u16 {
        self.columns.saturating_mul(self.rows)
    }

    pub(crate) fn corner(self, index: u16) -> Option<GuiPoint> {
        if index >= self.len() {
            return None;
        }
        let column = index.checked_rem(self.columns)?;
        let row = index.checked_div(self.columns)?;
        Some(GuiPoint::new(
            self.origin.x + SLOT_PITCH * i32::from(column),
            self.origin.y + SLOT_PITCH * i32::from(row),
        ))
    }
}
