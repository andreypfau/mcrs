use std::mem;

use bevy::prelude::*;
use bevy::render::MainWorld;

use crate::glyph_uploads::GlyphUploads;
use crate::gui_batch::GuiBatch;

fn main_batch_changed(main: &World) -> bool {
    main.get_resource_ref::<GuiBatch>()
        .is_some_and(|batch| batch.is_changed())
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
pub(crate) fn gui_batch_extract_due(main: Res<MainWorld>) -> bool {
    main_batch_changed(&main)
        || main
            .get_resource::<GlyphUploads>()
            .is_some_and(|uploads| !uploads.cells.is_empty())
}

pub(crate) fn extract_gui_batch(
    mut main: ResMut<MainWorld>,
    mut batch: ResMut<GuiBatch>,
    mut uploads: ResMut<GlyphUploads>,
) {
    if main_batch_changed(&main)
        && let Some(mut built) = main.get_resource_mut::<GuiBatch>()
    {
        mem::swap(built.bypass_change_detection(), &mut *batch);
    }
    if let Some(mut queued) = main.get_resource_mut::<GlyphUploads>()
        && !queued.cells.is_empty()
    {
        uploads
            .cells
            .append(&mut queued.bypass_change_detection().cells);
    }
}
