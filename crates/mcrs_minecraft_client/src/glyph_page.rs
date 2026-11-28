use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::math::{Rect, UVec2, Vec2};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mcrs_minecraft_client_gui::inflate_zip::inflate_zip;
use mcrs_minecraft_client_gui::sign_text::SignQuad;
use mcrs_minecraft_font::font::Font;
use mcrs_minecraft_font::font_definition::FontOptions;
use mcrs_minecraft_font::font_glyph::{ImageSource, MISSING_GLYPH_SIZE, missing_glyph_pixels};
use mcrs_minecraft_font::glyph_cache::{self, CELL, COLUMNS, ROWS, SLOTS};
use mcrs_minecraft_font::language::ClientLanguage;

use crate::stream::BlockCatalog;

pub struct TextAssetsPlugin;

impl Plugin for TextAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            load_assets
                .run_if(resource_exists::<BlockCatalog>)
                .run_if(not(resource_exists::<TextAssets>)),
        );
    }
}

pub(crate) fn ensure_loaded(app: &mut App) {
    if !app.is_plugin_added::<TextAssetsPlugin>() {
        app.add_plugins(TextAssetsPlugin);
    }
}

#[derive(Resource)]
pub(crate) struct TextAssets {
    pub(crate) font: Font,
    pub(crate) language: ClientLanguage,
    pub(crate) page: GlyphPage,
    pub(crate) image: Handle<Image>,
}

pub(crate) struct GlyphPage {
    width: u32,
    pub(crate) size: Vec2,
    bitmaps: Vec<(usize, Vec2)>,
    missing: Vec2,
    unifont: Vec2,
    cells: HashMap<char, usize>,
    pub(crate) pending: Vec<(usize, char)>,
}

impl GlyphPage {
    fn new(font: &Font) -> (Self, Vec<u8>, u32) {
        let region = UVec2::new((COLUMNS * CELL) as u32, (ROWS * CELL) as u32);
        let missing = UVec2::from(MISSING_GLYPH_SIZE);
        let bitmaps: Vec<_> = font.bitmap_pages().collect();
        let width = bitmaps
            .iter()
            .map(|(_, page)| page.size().0)
            .chain([region.x, missing.x])
            .max()
            .unwrap_or(region.x);
        let height =
            bitmaps.iter().map(|(_, page)| page.size().1).sum::<u32>() + missing.y + region.y;
        let mut pixels = vec![0; (width * height * 4) as usize];
        let mut y = 0;
        let mut place = |pixels: &mut Vec<u8>, source: &[u8], size: UVec2| {
            let at = UVec2::new(0, y);
            blit(pixels, width, at, source, size.x);
            y += size.y;
            at.as_vec2()
        };
        let bitmaps = bitmaps
            .into_iter()
            .map(|(index, page)| {
                let size = UVec2::from(page.size());
                (index, place(&mut pixels, page.pixels(), size))
            })
            .collect();
        let missing = place(&mut pixels, &missing_glyph_pixels(), missing);
        let unifont = place(&mut pixels, &[], region);
        let page = Self {
            width,
            size: Vec2::new(width as f32, height as f32),
            bitmaps,
            missing,
            unifont,
            cells: HashMap::new(),
            pending: Vec::new(),
        };
        (page, pixels, height)
    }

    fn texels(&mut self, source: ImageSource, glyph: char) -> Option<Rect> {
        let (min, size) = match source {
            ImageSource::Bitmap { source, texels } => {
                let (_, origin) = self.bitmaps.iter().find(|(page, _)| *page == source)?;
                return Some(Rect::from_corners(
                    texels.min + *origin,
                    texels.max + *origin,
                ));
            }
            ImageSource::Missing => (self.missing, UVec2::from(MISSING_GLYPH_SIZE).as_vec2()),
            ImageSource::Unifont { columns, .. } => {
                let cell = match self.cells.get(&glyph) {
                    Some(&cell) => cell,
                    None if self.cells.len() < SLOTS => {
                        let cell = self.cells.len();
                        self.cells.insert(glyph, cell);
                        self.pending.push((cell, glyph));
                        cell
                    }
                    None => return None,
                };
                let (x, y) = glyph_cache::cell_origin(cell);
                (
                    self.unifont + Vec2::new(x as f32, y as f32),
                    Vec2::new(columns.min(CELL as u32) as f32, CELL as f32),
                )
            }
        };
        Some(Rect::from_corners(min, min + size))
    }

    fn write_pending(&mut self, font: &Font, pixels: &mut [u8]) {
        for (cell, glyph) in self.pending.drain(..) {
            let Some(cell_pixels) = font.unifont_cell(glyph) else {
                continue;
            };
            let (x, y) = glyph_cache::cell_origin(cell);
            let at = self.unifont.as_uvec2() + UVec2::new(x, y);
            blit(pixels, self.width, at, &cell_pixels, CELL as u32);
        }
    }
}

fn blit(target: &mut [u8], stride: u32, at: UVec2, source: &[u8], width: u32) {
    let row = width as usize * 4;
    for (line, source_row) in source.chunks_exact(row).enumerate() {
        let start = ((at.y as usize + line) * stride as usize + at.x as usize) * 4;
        if let Some(target) = target.get_mut(start..start + row) {
            target.copy_from_slice(source_row);
        }
    }
}

fn load_assets(
    mut commands: Commands,
    catalog: Res<BlockCatalog>,
    mut images: ResMut<Assets<Image>>,
    mut failed: Local<bool>,
) {
    if *failed {
        return;
    }
    let Some(pack) = catalog.pack() else {
        return;
    };
    let loaded = (|| {
        let loaded = Font::load(pack.files(), FontOptions::default(), inflate_zip)
            .map_err(|error| error.to_string())?;
        for notice in &loaded.notices {
            warn!("{notice}");
        }
        let font = loaded.font;
        let (page, pixels, height) = GlyphPage::new(&font);
        let mut image = Image::new(
            Extent3d {
                width: page.width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::nearest();
        let language = ClientLanguage::load(pack.files()).map_err(|error| error.to_string())?;
        Ok::<_, String>(TextAssets {
            font,
            language,
            page,
            image: images.add(image),
        })
    })();
    match loaded {
        Ok(assets) => commands.insert_resource(assets),
        Err(reason) => {
            *failed = true;
            error!("the world text cannot load its font: {reason}");
        }
    }
}

pub(crate) fn linear(argb: u32) -> [f32; 3] {
    let color = Color::srgb_u8((argb >> 16) as u8, (argb >> 8) as u8, argb as u8).to_linear();
    [color.red, color.green, color.blue]
}

pub(crate) fn lightmap_levels(light: u32) -> [f32; 2] {
    [(light & 0xFF) as f32, ((light >> 20) & 15) as f32 * 16.0]
}

impl GlyphPage {
    pub(crate) fn uv_corners(&mut self, quad: &SignQuad) -> Option<[[f32; 2]; 4]> {
        let texels = self.texels(quad.source, quad.glyph)?;
        let uv = |x: f32, y: f32| [x / self.size.x, y / self.size.y];
        let (min, max) = (texels.min, texels.max);
        Some([
            uv(min.x, min.y),
            uv(min.x, max.y),
            uv(max.x, max.y),
            uv(max.x, min.y),
        ])
    }
}

impl TextAssets {
    pub(crate) fn flush(&mut self, images: &mut Assets<Image>) {
        if !self.page.pending.is_empty()
            && let Some(mut image) = images.get_mut(&self.image)
            && let Some(pixels) = image.data.as_mut()
        {
            self.page.write_pending(&self.font, pixels);
        }
    }
}
