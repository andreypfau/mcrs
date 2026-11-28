use bevy::math::{IRect, IVec2, Rect};
use bevy::prelude::*;
use mcrs_minecraft_font::{GlyphImage, Weight};

pub const WHITE: u32 = 0xFFFF_FFFF;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuiQuad {
    Sprite {
        rect: IRect,
        region: &'static str,
        color: u32,
    },
    Fill {
        rect: IRect,
        color: u32,
    },
    FillGradient {
        rect: IRect,
        top: u32,
        bottom: u32,
    },
    Texels {
        rect: IRect,
        texels: Rect,
        color: u32,
    },
    Glyph {
        pen: Vec2,
        glyph: char,
        image: GlyphImage,
        color: u32,
        weight: Weight,
    },
    Icon {
        origin: IVec2,
        index: usize,
    },
    Highlight {
        rect: IRect,
        invert: bool,
    },
    Clip {
        rect: IRect,
    },
    EndClip,
    Figure {
        rect: IRect,
    },
}

pub(crate) fn sprite(at: IVec2, size: IVec2, region: &'static str) -> GuiQuad {
    GuiQuad::Sprite {
        rect: IRect::from_corners(at, at + size),
        region,
        color: WHITE,
    }
}
