use core::mem;

use bevy::prelude::*;
use mcrs_minecraft_container_gesture::DragPreview;

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ShownDragPreview {
    preview: DragPreview,
}

impl ShownDragPreview {
    #[must_use]
    pub fn preview(&self) -> &DragPreview {
        &self.preview
    }

    pub(crate) fn show(&mut self, preview: &mut DragPreview) {
        mem::swap(&mut self.preview, preview);
    }
}
