use bevy::math::IVec2;
use bevy::prelude::*;

#[derive(Resource, Clone, Copy)]
pub struct GuiConfig {
    pub scale: u32,
    pub cursor: Option<IVec2>,
    pub frozen_ticks: Option<i64>,
    pub advanced_tooltips: bool,
}
