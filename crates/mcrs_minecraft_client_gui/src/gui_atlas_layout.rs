use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;
use crate::sprite::GuiSprite;
use bevy::math::{IRect, IVec2, Rect, Vec2};
use bevy::prelude::*;
use mcrs_minecraft_font::{CELL, GlyphCache, ImageSource, cell_origin};
use std::collections::HashMap;

#[derive(Resource, Clone, Debug)]
pub struct GuiAtlasLayout {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) regions: HashMap<&'static str, IRect>,
    pub(crate) sprites: [GuiSprite; GuiSpriteId::ALL.len()],
    pub(crate) textures: [IRect; GuiTextureId::ALL.len()],
    pub(crate) missing: GuiSprite,
    pub(crate) page_origins: Vec<(usize, IVec2)>,
    pub(crate) missing_glyph: IRect,
    pub(crate) glyph_origin: IVec2,
    pub(crate) chat_modified: IRect,
}

impl GuiAtlasLayout {
    pub fn region(&self, name: &str) -> IRect {
        self.regions.get(name).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn sprite(&self, id: GuiSpriteId) -> GuiSprite {
        self.sprites
            .get(id as usize)
            .copied()
            .unwrap_or(self.missing)
    }

    pub(crate) fn uv(&self, pixel: IRect) -> [[f32; 2]; 4] {
        self.texel_uv(pixel.as_rect())
    }

    pub(crate) fn glyph_texels(
        &self,
        source: ImageSource,
        glyph: char,
        cache: &mut GlyphCache,
    ) -> Option<Rect> {
        match source {
            ImageSource::Bitmap { source, texels } => {
                let (_, origin) = self.page_origins.iter().find(|(page, _)| *page == source)?;
                Some(Rect::from_corners(
                    texels.min + origin.as_vec2(),
                    texels.max + origin.as_vec2(),
                ))
            }
            ImageSource::Missing => Some(self.missing_glyph.as_rect()),
            ImageSource::Unifont { columns, .. } => {
                let (x, y) = cell_origin(cache.cell(glyph)?);
                let min = self.glyph_origin.as_vec2() + Vec2::new(x as f32, y as f32);
                Some(Rect::from_corners(
                    min,
                    min + Vec2::new(columns.min(CELL as u32) as f32, CELL as f32),
                ))
            }
        }
    }

    pub(crate) fn texel_uv(&self, texels: Rect) -> [[f32; 2]; 4] {
        let (w, h) = (self.width as f32, self.height as f32);
        let (x0, y0, x1, y1) = (
            texels.min.x / w,
            texels.min.y / h,
            texels.max.x / w,
            texels.max.y / h,
        );
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }
}
