use bevy::math::IRect;
use bevy::prelude::*;
use mcrs_minecraft_item_icon::GuiVertex;
use std::ops::Range;

pub const GUI_ATLAS_BIT: u32 = 1 << 31;
pub const GLINT_BIT: u32 = 1 << 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiDraw {
    pub range: Range<u32>,
    pub kind: GuiDrawKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiDrawKind {
    Flat,
    Item,
    InvertRedGreen,
    FullBlue,
    Figure { rect: IRect },
}

#[derive(Resource, Default)]
pub struct GuiBatch {
    pub vertices: Vec<GuiVertex>,
    pub draws: Vec<GuiDraw>,
    pub scale: u32,
}
