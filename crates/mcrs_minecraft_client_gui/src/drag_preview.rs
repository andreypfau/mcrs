use bevy::prelude::*;
use mcrs_minecraft_inventory_click::{DragEntry, DragSplit, EntryPreview};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreviewEntry {
    index: u16,
    entry: DragEntry,
    preview: Option<EntryPreview>,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct DragPreview {
    entries: Vec<PreviewEntry>,
    cursor: Option<u8>,
}

impl DragPreview {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = None;
    }

    pub fn join(&mut self, index: u16, entry: DragEntry) {
        self.entries.push(PreviewEntry {
            index,
            entry,
            preview: None,
        });
    }

    #[must_use]
    pub fn entry_count(&self) -> u32 {
        u32::try_from(self.entries.len()).unwrap_or(u32::MAX)
    }

    pub fn split(&mut self, split: DragSplit) {
        for joined in &mut self.entries {
            joined.preview = split.entry(joined.entry);
        }
        self.cursor = Some(split.cursor(self.entries.iter().map(|joined| joined.entry)));
    }

    #[must_use]
    pub fn entry(&self, index: u16) -> Option<EntryPreview> {
        let joined = self.entries.iter().find(|joined| joined.index == index)?;
        joined.preview
    }

    #[must_use]
    pub fn cursor(&self) -> Option<u8> {
        self.cursor
    }
}
