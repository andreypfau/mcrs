use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;
use bevy::time::Real;

use crate::gui_config::GuiConfig;
use crate::menu_input;

#[derive(Resource, Clone, Copy, Default, PartialEq, ExtractResource)]
#[extract_app(RenderApp)]
pub(crate) struct GuiGlint {
    pub(crate) offset: [f32; 2],
}

pub(crate) fn write_gui_glint(
    time: Res<Time<Real>>,
    config: Res<GuiConfig>,
    mut glint: ResMut<GuiGlint>,
) {
    let millis = config.frozen_ticks.map_or(
        i64::try_from(menu_input::menu_clock(&time)).unwrap_or(i64::MAX),
        |ticks| ticks.saturating_mul(50),
    );
    glint.set_if_neq(GuiGlint {
        offset: glint_offset(millis),
    });
}

pub(crate) const GLINT_ALPHA: f32 = 0.75;
pub(crate) const GLINT_SPEED: f64 = 0.5;

pub(crate) fn glint_offset(millis: i64) -> [f32; 2] {
    let ms = (millis as f64 * GLINT_SPEED * 8.0).floor() as i64;
    [
        (ms.rem_euclid(110_000)) as f32 / 110_000.0,
        (ms.rem_euclid(30_000)) as f32 / 30_000.0,
    ]
}
