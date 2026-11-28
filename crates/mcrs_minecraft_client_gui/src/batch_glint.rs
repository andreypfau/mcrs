use bevy::prelude::*;

#[derive(Resource, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BatchGlint {
    pub(crate) has_glint: bool,
}

pub(crate) fn batch_has_glint(glint: Res<BatchGlint>) -> bool {
    glint.has_glint
}
