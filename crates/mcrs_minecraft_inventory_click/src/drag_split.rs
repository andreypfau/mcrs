use mcrs_minecraft_inventory_transaction::Planned;

use crate::DragEntry;
use crate::DragKind;
use crate::EntryPreview;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DragSplit {
    pub kind: DragKind,
    pub count: u8,
    pub max: u8,
    pub size: u32,
}

impl DragSplit {
    #[must_use]
    pub fn from_carried<T>(kind: DragKind, carried: &Planned<T>, size: u32) -> Self {
        Self {
            kind,
            count: carried.count,
            max: carried.max,
            size,
        }
    }

    #[must_use]
    pub fn entry(self, entry: DragEntry) -> Option<EntryPreview> {
        let portion = self.portion()?;
        let filled = entry.held.saturating_add(portion).min(entry.cap);
        if u16::from(entry.held) + u16::from(portion) > u16::from(entry.cap) {
            Some(EntryPreview::Capped(filled))
        } else {
            Some(EntryPreview::Fits(filled))
        }
    }

    #[must_use]
    pub fn cursor(self, entries: impl IntoIterator<Item = DragEntry>) -> u8 {
        if self.kind == DragKind::Full && self.portion().is_some() {
            return self.max;
        }
        let mut left = i32::from(self.count);
        for entry in entries {
            let Some(preview) = self.entry(entry) else {
                return self.count;
            };
            let gain = i32::from(preview.count()) - i32::from(entry.held);
            left = left.saturating_sub(gain);
        }
        u8::try_from(left.clamp(0, i32::from(u8::MAX))).unwrap_or(u8::MAX)
    }

    fn portion(self) -> Option<u8> {
        if self.size < 2 {
            return None;
        }
        Some(match self.kind {
            DragKind::Split => {
                let part = u32::from(self.count).checked_div(self.size).unwrap_or(0);
                u8::try_from(part).unwrap_or(u8::MAX)
            }
            DragKind::Single => 1,
            DragKind::Full => self.max,
        })
    }
}
