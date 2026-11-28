use bevy::prelude::*;

#[derive(Resource)]
pub(crate) struct GuiImageHandles {
    pub(crate) atlas: Handle<Image>,
    pub(crate) glint: Handle<Image>,
}
