use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use bevy::ecs::resource::IsResource;
use bevy::ecs::world::EntityRef;
use bevy::math::{IRect, IVec2, Rect, UVec2};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_item::{
    ItemStack, SelectedHotbarSlot, SlotTable, damage_value, is_damaged, max_damage,
};

use crate::atlas_quad::AtlasQuad;
use crate::font::Font;
use crate::font_definition::FontOptions;
use crate::font_glyph::{ImageSource, MISSING_GLYPH_SIZE, Weight, missing_glyph_pixels};
use crate::glyph_cache::{self, GlyphCache};
use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;
use crate::hotbar::hotbar;
use crate::inventory_screen::inventory_screen;
use crate::item_decorations::{Decorated, WHITE, decorations};
use crate::item_resolve::{GuiVertex, ItemRenderLayers};
use crate::language::ClientLanguage;
use crate::sprite::{GuiSprite, Scaling};
use crate::tooltip::Tooltip;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_model::atlas::{MISSING_SIZE, decode_png, missing_pixels, rgba};
use mcrs_minecraft_model::resource_set::ResourceSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuiSize {
    pub width: i32,
    pub height: i32,
    pub scale: u32,
}

impl GuiSize {
    pub fn auto_scale(framebuffer: UVec2) -> u32 {
        (framebuffer.x / 320).min(framebuffer.y / 240).max(1)
    }

    /// The GUI size for the option value `scale`, where 0 is Auto and a value above the scale of
    /// Auto gives that scale (MENU-SCALE-001).
    pub fn of(framebuffer: UVec2, scale: u32) -> Self {
        let auto = Self::auto_scale(framebuffer);
        let scale = if scale == 0 { auto } else { scale.min(auto) };
        Self {
            width: framebuffer.x.div_ceil(scale) as i32,
            height: framebuffer.y.div_ceil(scale) as i32,
            scale,
        }
    }
}

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
    },
    /// One strike of a glyph with its pen at `pen`, in GUI units.
    Glyph {
        pen: Vec2,
        glyph: char,
        color: u32,
        weight: Weight,
    },
    Item {
        origin: IVec2,
        stack: Entity,
    },
    /// Changes the colour of each pixel already drawn in `rect`: to (255 − R, 255 − G, 255) with
    /// `invert`, else to (R, G, 255) (EDIT-SEL-001).
    Highlight {
        rect: IRect,
        invert: bool,
    },
    /// Cuts the quads that follow, up to the matching `EndClip`, to `rect` and to each clip
    /// around it.
    Clip {
        rect: IRect,
    },
    /// Ends the innermost `Clip`.
    EndClip,
}

/// What the screens emit each frame, in draw order.
#[derive(Resource, Default)]
pub struct GuiScene {
    pub size: GuiSize,
    pub cursor: IVec2,
    pub quads: Vec<GuiQuad>,
    pub tooltip: Tooltip,
}

#[derive(Resource, Clone, Copy)]
pub struct GuiConfig {
    pub scale: u32,
    pub cursor: Option<IVec2>,
    pub frozen_ticks: Option<i64>,
    pub advanced_tooltips: bool,
}

const ATLAS_WIDTH: u32 = 512;
const SWITCHER_WIDTH: u32 = 125;
const SWITCHER_HEIGHT: u32 = 75;
const BLANK: &str = "blank";
const MISSING: &str = "missing";
const MISSING_GLYPH: &str = "font/missing";
const CELL: usize = crate::font_unihex::CELL;

const GUI_TEXTURES: [&str; 18] = [
    "hud/hotbar",
    "hud/hotbar_selection",
    "hud/hotbar_offhand_left",
    "container/slot_highlight_back",
    "container/slot_highlight_front",
    "container/slot/helmet",
    "container/slot/chestplate",
    "container/slot/leggings",
    "container/slot/boots",
    "container/slot/shield",
    "recipe_book/button",
    "recipe_book/button_highlighted",
    "container/inventory",
    "container/gamemode_switcher",
    "gamemode_switcher/slot",
    "gamemode_switcher/selection",
    "misc/enchanted_glint_item",
    "icon/chat_modified",
];

fn texture_path(name: &str) -> String {
    match name {
        "container/inventory" | "container/gamemode_switcher" => {
            format!("minecraft/textures/gui/{name}.png")
        }
        "misc/enchanted_glint_item" => format!("minecraft/textures/{name}.png"),
        _ => format!("minecraft/textures/gui/sprites/{name}.png"),
    }
}

pub struct GuiAtlasData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub regions: HashMap<&'static str, IRect>,
    pub font: Font,
    pub glint: (u32, u32, Vec<u8>),
    pub(crate) sprites: [GuiSprite; GuiSpriteId::ALL.len()],
    pub(crate) textures: [IRect; GuiTextureId::ALL.len()],
    pub missing: GuiSprite,
    /// Where the texture of each bitmap provider of the font lies, by the provider's index.
    pub page_origins: Vec<(usize, IVec2)>,
    pub missing_glyph: IRect,
    /// The top left of the region the glyph cache draws Unifont glyphs into.
    pub glyph_origin: IVec2,
    pub chat_modified: IRect,
}

/// What a packed image is, so the font's textures need no name of their own.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Packed {
    Named(&'static str),
    Page(usize),
}

#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GuiAtlas(pub Arc<GuiAtlasData>);

impl GuiAtlasData {
    /// Shelf-packs the GUI textures the screens blit, which are not square and so
    /// bypass the sprite arrays; the glint stays separate because it repeats.
    #[expect(
        clippy::too_many_lines,
        reason = "one load packs every texture in the order each placement depends on the last"
    )]
    pub fn load(pack: &ResourceSet) -> Result<Self, String> {
        let mut images = Vec::new();
        let mut glint = None;
        let sprite_scalings = load_widget_images(pack, &mut images);
        for name in GUI_TEXTURES {
            let path = texture_path(name);
            let (pixels, width, height) =
                decode_png(pack.read(&path).map_err(|error| error.to_string())?, &path)
                    .map_err(|error| error.to_string())?;
            match name {
                "misc/enchanted_glint_item" => glint = Some((width, height, pixels)),
                "container/inventory" => images.push((
                    Packed::Named(name),
                    crop(&pixels, width, 176, 166),
                    176,
                    166,
                )),
                "container/gamemode_switcher" => images.push((
                    Packed::Named(name),
                    crop(&pixels, width, SWITCHER_WIDTH, SWITCHER_HEIGHT),
                    SWITCHER_WIDTH,
                    SWITCHER_HEIGHT,
                )),
                _ => images.push((Packed::Named(name), pixels, width, height)),
            }
        }
        let font = Font::load(pack, FontOptions::default())?;
        for (source, page) in font.bitmap_pages() {
            let (width, height) = page.size();
            images.push((Packed::Page(source), page.pixels().to_vec(), width, height));
        }
        images.push((Packed::Named(BLANK), vec![255; 16], 2, 2));
        images.push((
            Packed::Named(MISSING),
            missing_pixels(),
            MISSING_SIZE,
            MISSING_SIZE,
        ));
        images.push((
            Packed::Named(MISSING_GLYPH),
            missing_glyph_pixels(),
            MISSING_GLYPH_SIZE.0,
            MISSING_GLYPH_SIZE.1,
        ));
        images.sort_by_key(|(name, _, _, height)| (std::cmp::Reverse(*height), *name));
        let (mut x, mut y, mut row) = (0u32, 0u32, 0u32);
        let mut placed = Vec::new();
        for (name, pixels, width, height) in images {
            if x + width > ATLAS_WIDTH {
                x = 0;
                y += row;
                row = 0;
            }
            placed.push((
                name,
                pixels,
                IRect::new(x as i32, y as i32, (x + width) as i32, (y + height) as i32),
            ));
            x += width;
            row = row.max(height);
        }
        let glyph_origin = IVec2::new(0, (y + row) as i32);
        let atlas_height = (y + row + (glyph_cache::ROWS * CELL) as u32).next_power_of_two();
        let mut pixels = vec![0u8; (ATLAS_WIDTH * atlas_height * 4) as usize];
        let mut regions = HashMap::new();
        let mut page_origins = Vec::new();
        for (name, source, rect) in placed {
            let width = rect.width() as usize;
            for (line, source_row) in source.chunks_exact(width * 4).enumerate() {
                let start =
                    ((rect.min.y as usize + line) * ATLAS_WIDTH as usize + rect.min.x as usize) * 4;
                if let Some(target) = pixels.get_mut(start..start + width * 4) {
                    target.copy_from_slice(source_row);
                }
            }
            match name {
                Packed::Named(name) => {
                    regions.insert(name, rect);
                }
                Packed::Page(source) => page_origins.push((source, rect.min)),
            }
        }
        let missing_region = regions.get(MISSING).copied().unwrap_or_default();
        let missing = GuiSprite::new(missing_region, Scaling::Stretch);
        let mut sprites = [missing; GuiSpriteId::ALL.len()];
        for (id, scaling) in sprite_scalings {
            let region = regions.get(id.name());
            if let (Some(slot), Some(&region), Some(scaling)) =
                (sprites.get_mut(id as usize), region, scaling)
            {
                *slot = GuiSprite::new(region, scaling);
            }
        }
        let mut textures = [missing_region; GuiTextureId::ALL.len()];
        for id in GuiTextureId::ALL {
            if let (Some(slot), Some(&region)) =
                (textures.get_mut(id as usize), regions.get(id.name()))
            {
                *slot = region;
            }
        }
        let missing_glyph = regions.get(MISSING_GLYPH).copied().unwrap_or_default();
        let chat_modified = regions
            .get("icon/chat_modified")
            .copied()
            .unwrap_or_default();
        Ok(Self {
            width: ATLAS_WIDTH,
            height: atlas_height,
            pixels,
            regions,
            font,
            glint: glint.ok_or("the GUI atlas lists no glint")?,
            sprites,
            textures,
            missing,
            page_origins,
            missing_glyph,
            glyph_origin,
            chat_modified,
        })
    }

    pub fn region(&self, name: &str) -> IRect {
        self.regions.get(name).copied().unwrap_or_default()
    }

    /// The sprite `id` with its scaling, or the missing sprite when the pack lacks it or it
    /// failed to load.
    #[must_use]
    pub fn sprite(&self, id: GuiSpriteId) -> GuiSprite {
        self.sprites
            .get(id as usize)
            .copied()
            .unwrap_or(self.missing)
    }

    fn uv(&self, pixel: IRect) -> [[f32; 2]; 4] {
        self.texel_uv(pixel.as_rect())
    }

    /// Where the picture of a glyph lies in the atlas. A Unifont glyph takes a cell of the glyph
    /// region, which is `None` when the cache has none to give.
    fn glyph_texels(
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
                let (x, y) = glyph_cache::cell_origin(cache.cell(glyph)?);
                let min = self.glyph_origin.as_vec2() + Vec2::new(x as f32, y as f32);
                Some(Rect::from_corners(
                    min,
                    min + Vec2::new(columns.min(CELL as u32) as f32, CELL as f32),
                ))
            }
        }
    }

    fn texel_uv(&self, texels: Rect) -> [[f32; 2]; 4] {
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

/// Adds the textures and the sprites of the widgets to `images`, and gives the scaling of each
/// sprite that loaded.
fn load_widget_images(
    pack: &ResourceSet,
    images: &mut Vec<(Packed, Vec<u8>, u32, u32)>,
) -> [(GuiSpriteId, Option<Scaling>); GuiSpriteId::ALL.len()] {
    for id in GuiTextureId::ALL {
        let path = format!("minecraft/{}", id.name());
        load_image(pack, id.name(), &path, Ok(Scaling::Stretch), images);
    }
    GuiSpriteId::ALL.map(|id| {
        let path = texture_path(id.name());
        let scaling = Scaling::read(pack.get(&format!("{path}.mcmeta")));
        (id, load_image(pack, id.name(), &path, scaling, images))
    })
}

/// Adds the image `minecraft:name` at `path` to `images` and gives its scaling. An image the pack
/// lacks, or one with an error, is left out, so it draws the missing sprite.
fn load_image(
    pack: &ResourceSet,
    name: &'static str,
    path: &str,
    scaling: Result<Scaling, String>,
    images: &mut Vec<(Packed, Vec<u8>, u32, u32)>,
) -> Option<Scaling> {
    let bytes = pack.get(path)?;
    let loaded = scaling.and_then(|scaling| {
        let image = decode_png(bytes, path).map_err(|error| error.to_string())?;
        // chisle: the atlas has a fixed width, and the packer would wrap a wider image into the
        // rows below it; a pack with such an image needs an atlas that grows in width.
        if image.1 > ATLAS_WIDTH {
            return Err(format!(
                "{} pixels wide, the atlas holds {ATLAS_WIDTH}",
                image.1
            ));
        }
        Ok((scaling, image))
    });
    match loaded {
        Ok((scaling, (pixels, width, height))) => {
            images.push((Packed::Named(name), pixels, width, height));
            Some(scaling)
        }
        Err(reason) => {
            error!("GUI image minecraft:{name}: {reason}");
            None
        }
    }
}

fn crop(pixels: &[u8], stride: u32, width: u32, height: u32) -> Vec<u8> {
    let stride = stride as usize * 4;
    let width = width as usize * 4;
    (0..height as usize)
        .flat_map(|row| {
            pixels
                .get(row * stride..row * stride + width)
                .into_iter()
                .flatten()
                .copied()
        })
        .collect()
}

pub const GUI_ATLAS_BIT: u32 = 1 << 31;
pub const GLINT_BIT: u32 = 1 << 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiDraw {
    pub range: Range<u32>,
    pub kind: GuiDrawKind,
}

/// How the pass draws the vertices of a [`GuiDraw`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiDrawKind {
    /// Atlas quads, blended over what is drawn.
    Flat,
    /// The layers of one item, depth-tested in a band of their own.
    Item,
    /// White quads that turn the red and the green of what is drawn into one minus their value.
    InvertRedGreen,
    /// Quads that set the blue of what is drawn to full.
    FullBlue,
}

/// The scene flattened to what the pass draws: triangle-list vertices in GUI units
/// and the draws over them, one per item element.
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GuiBatch {
    pub vertices: Vec<GuiVertex>,
    pub draws: Vec<GuiDraw>,
    pub scale: u32,
    pub glint_offset: [f32; 2],
    /// The Unifont glyph in each cell of the atlas's glyph region.
    pub glyphs: Vec<Option<char>>,
}

pub const GLINT_ALPHA: f32 = 0.75;
const GLINT_SPEED: f64 = 0.5;

pub fn glint_offset(millis: i64) -> [f32; 2] {
    let ms = (millis as f64 * GLINT_SPEED * 8.0).floor() as i64;
    [
        (ms.rem_euclid(110_000)) as f32 / 110_000.0,
        (ms.rem_euclid(30_000)) as f32 / 30_000.0,
    ]
}

pub(super) fn sprite(at: IVec2, size: IVec2, region: &'static str) -> GuiQuad {
    GuiQuad::Sprite {
        rect: IRect::from_corners(at, at + size),
        region,
        color: WHITE,
    }
}

fn push_quad(vertices: &mut Vec<GuiVertex>, quad: AtlasQuad, sprite: u32) {
    let AtlasQuad { rect, uv, colors } = quad;
    let corners = [
        [rect.min.x, rect.min.y, 0.0],
        [rect.max.x, rect.min.y, 0.0],
        [rect.max.x, rect.max.y, 0.0],
        [rect.min.x, rect.max.y, 0.0],
    ];
    let vertex = |pos: [f32; 3], uv: [f32; 2], color: u32| GuiVertex {
        pos,
        uv,
        color: rgba(color),
        sprite,
    };
    let [c0, c1, c2, c3] = corners;
    let [u0, u1, u2, u3] = uv;
    let [k0, k1, k2, k3] = colors;
    let (a, b, c, d) = (
        vertex(c0, u0, k0),
        vertex(c1, u1, k1),
        vertex(c2, u2, k2),
        vertex(c3, u3, k3),
    );
    vertices.extend([a, b, c, a, c, d]);
}

/// The window pixels, at least 1 × 1.
pub(crate) fn window_framebuffer(window: &Window) -> UVec2 {
    UVec2::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
    )
}

fn begin_gui_frame(
    window: Single<&Window, With<PrimaryWindow>>,
    config: Res<GuiConfig>,
    mut scene: ResMut<GuiScene>,
) {
    scene.size = GuiSize::of(window_framebuffer(&window), config.scale);
    scene.cursor = config.cursor.unwrap_or_else(|| {
        window
            .physical_cursor_position()
            .map_or(IVec2::new(-1, -1), |p| {
                (p / scene.size.scale as f32).floor().as_ivec2()
            })
    });
    scene.quads.clear();
    scene.tooltip.clear();
}

fn draw_tooltip(atlas: Option<Res<GuiAtlas>>, mut scene: ResMut<GuiScene>) {
    let Some(atlas) = atlas else {
        return;
    };
    let scene = &mut *scene;
    crate::tooltip::draw(&scene.tooltip, scene.size, &atlas.0, &mut scene.quads);
}

fn draw_screens(
    screen: Res<Screen>,
    player: Query<(&SlotTable, &SelectedHotbarSlot), With<Player>>,
    mut scene: ResMut<GuiScene>,
) {
    let scene = &mut *scene;
    let Ok((slots, selected)) = player.single() else {
        return;
    };
    hotbar(scene.size, selected.0, slots, &mut scene.quads);
    if *screen == Screen::Inventory {
        inventory_screen(scene.size, scene.cursor, slots, &mut scene.quads);
    }
}

/// Loads the GUI textures and the translations from `pack`, or says which of them it lacks.
pub fn load_gui_atlas(pack: &ResourceSet, commands: &mut Commands) -> Result<(), String> {
    let atlas = GuiAtlasData::load(pack).map_err(|reason| format!("the GUI textures: {reason}"))?;
    let language =
        ClientLanguage::load(pack).map_err(|reason| format!("the translations: {reason}"))?;
    commands.insert_resource(GuiAtlas(Arc::new(atlas)));
    commands.insert_resource(language);
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "a Bevy system: each resource and query is its own parameter, and one pass turns each kind of quad into vertices"
)]
fn build_gui_batch(
    scene: Res<GuiScene>,
    atlas: Option<Res<GuiAtlas>>,
    layers: Query<&ItemRenderLayers>,
    stacks: Query<EntityRef, (With<ItemStack>, Without<IsResource>)>,
    time: Res<Time<Real>>,
    config: Res<GuiConfig>,
    mut cache: ResMut<GlyphCache>,
    mut batch: ResMut<GuiBatch>,
    mut clips: Local<Vec<IRect>>,
) {
    let batch = &mut *batch;
    cache.begin_frame();
    clips.clear();
    batch.vertices.clear();
    batch.draws.clear();
    batch.scale = scene.size.scale;
    let millis = config
        .frozen_ticks
        .map_or(time.elapsed().as_millis() as i64, |ticks| ticks * 50);
    batch.glint_offset = glint_offset(millis);
    let Some(atlas) = atlas.as_deref() else {
        return;
    };
    let atlas = &*atlas.0;
    let blank = atlas.uv(atlas.region(BLANK));
    let mut expanded = Vec::with_capacity(scene.quads.len() * 2);
    for quad in &scene.quads {
        match *quad {
            GuiQuad::Item { origin, stack } => {
                expanded.push(*quad);
                let Ok(stack) = stacks.get(stack) else {
                    continue;
                };
                let decorated = Decorated {
                    count: stack.get::<ItemStack>().map_or(1, |stack| stack.count),
                    damage: is_damaged(stack).then(|| (damage_value(stack), max_damage(stack))),
                };
                decorations(origin, &decorated, &atlas.font, &mut expanded);
            }
            other => expanded.push(other),
        }
    }
    let mut flat_start = None;
    let close_flat = |start: &mut Option<u32>, end: u32, draws: &mut Vec<GuiDraw>| {
        if let Some(begin) = start.take()
            && begin != end
        {
            draws.push(GuiDraw {
                range: begin..end,
                kind: GuiDrawKind::Flat,
            });
        }
    };
    for quad in expanded {
        let vertices = &mut batch.vertices;
        match quad {
            GuiQuad::Item { origin, stack } => {
                let end = vertices.len() as u32;
                close_flat(&mut flat_start, end, &mut batch.draws);
                let Ok(layers) = layers.get(stack) else {
                    continue;
                };
                // chisle: the layers of an item ignore the clip, so an item in a scrolled list
                // shows across its edge; a clip of the item vertices lifts it.
                push_item_layers(vertices, layers, origin);
                let after = vertices.len() as u32;
                if after != end {
                    batch.draws.push(GuiDraw {
                        range: end..after,
                        kind: GuiDrawKind::Item,
                    });
                }
            }
            GuiQuad::Highlight { rect, invert } => {
                let end = vertices.len() as u32;
                close_flat(&mut flat_start, end, &mut batch.draws);
                let white = GuiQuad::Fill { rect, color: WHITE };
                push_flat_quad(
                    vertices,
                    atlas,
                    blank,
                    &mut cache,
                    white,
                    clips.last().copied(),
                );
                let range = end..vertices.len() as u32;
                if range.is_empty() {
                    continue;
                }
                if invert {
                    batch.draws.push(GuiDraw {
                        range: range.clone(),
                        kind: GuiDrawKind::InvertRedGreen,
                    });
                }
                batch.draws.push(GuiDraw {
                    range,
                    kind: GuiDrawKind::FullBlue,
                });
            }
            GuiQuad::Clip { rect } => {
                let inner = clips.last().map_or(rect, |outer| outer.intersect(rect));
                clips.push(inner);
            }
            GuiQuad::EndClip => {
                clips.pop();
            }
            flat => {
                if flat_start.is_none() {
                    flat_start = Some(vertices.len() as u32);
                }
                push_flat_quad(
                    vertices,
                    atlas,
                    blank,
                    &mut cache,
                    flat,
                    clips.last().copied(),
                );
            }
        }
    }
    let end = batch.vertices.len() as u32;
    close_flat(&mut flat_start, end, &mut batch.draws);
    batch.glyphs.clear();
    batch.glyphs.extend_from_slice(cache.resident());
}

fn push_item_layers(vertices: &mut Vec<GuiVertex>, layers: &ItemRenderLayers, origin: IVec2) {
    for layer in &layers.layers {
        let glint = if layer.foil { GLINT_BIT } else { 0 };
        for quad in layer.vertices.as_chunks::<4>().0 {
            let shifted = |v: &GuiVertex| GuiVertex {
                pos: [
                    v.pos[0] + origin.x as f32,
                    v.pos[1] + origin.y as f32,
                    v.pos[2],
                ],
                sprite: v.sprite | glint,
                ..*v
            };
            let [a, b, c, d] = quad.each_ref().map(shifted);
            vertices.extend([a, b, c, a, c, d]);
        }
    }
}

/// The vertices of a quad drawn from the GUI atlas, cut to `clip`; an item quad draws its own
/// layers instead.
fn push_flat_quad(
    vertices: &mut Vec<GuiVertex>,
    atlas: &GuiAtlasData,
    blank: [[f32; 2]; 4],
    cache: &mut GlyphCache,
    quad: GuiQuad,
    clip: Option<IRect>,
) {
    let flat = match quad {
        GuiQuad::Sprite {
            rect,
            region,
            color,
        } => AtlasQuad {
            rect: rect.as_rect(),
            uv: atlas.uv(atlas.region(region)),
            colors: [color; 4],
        },
        GuiQuad::Fill { rect, color } => AtlasQuad {
            rect: rect.as_rect(),
            uv: blank,
            colors: [color; 4],
        },
        GuiQuad::Texels { rect, texels } => AtlasQuad {
            rect: rect.as_rect(),
            uv: atlas.texel_uv(texels),
            colors: [WHITE; 4],
        },
        GuiQuad::FillGradient { rect, top, bottom } => AtlasQuad {
            rect: rect.as_rect(),
            uv: blank,
            colors: [top, top, bottom, bottom],
        },
        GuiQuad::Glyph {
            pen,
            glyph,
            color,
            weight,
        } => {
            let Some(image) = atlas.font.glyph(glyph).image else {
                return;
            };
            let Some(texels) = atlas.glyph_texels(image.source, glyph, cache) else {
                return;
            };
            AtlasQuad {
                rect: image.strike(pen, weight),
                uv: atlas.texel_uv(texels),
                colors: [color; 4],
            }
        }
        GuiQuad::Item { .. }
        | GuiQuad::Highlight { .. }
        | GuiQuad::Clip { .. }
        | GuiQuad::EndClip => return,
    };
    let clipped = match clip {
        Some(clip) => flat.clip(clip),
        None => Some(flat),
    };
    if let Some(flat) = clipped {
        push_quad(vertices, flat, GUI_ATLAS_BIT);
    }
}

pub struct GuiPlugin {
    pub config: GuiConfig,
    pub screen: Screen,
}

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config)
            .insert_resource(self.screen)
            .init_resource::<GuiScene>()
            .init_resource::<GuiBatch>()
            .init_resource::<GlyphCache>()
            .add_plugins(ExtractResourcePlugin::<GuiBatch>::default())
            .add_plugins(ExtractResourcePlugin::<GuiAtlas>::default())
            .add_systems(
                PostUpdate,
                (
                    begin_gui_frame,
                    draw_screens,
                    crate::chat::draw,
                    crate::game_mode_switcher::draw,
                    crate::menu_draw::draw.run_if(resource_exists::<crate::menu::Menu>),
                    draw_tooltip,
                    build_gui_batch,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_size_rounds_up_and_auto_scale_keeps_320x240() {
        assert_eq!(
            GuiSize::of(UVec2::new(855, 480), 2),
            GuiSize {
                width: 428,
                height: 240,
                scale: 2
            }
        );
        assert_eq!(GuiSize::auto_scale(UVec2::new(854, 480)), 2);
        assert_eq!(GuiSize::auto_scale(UVec2::new(1920, 1080)), 4);
        assert_eq!(GuiSize::auto_scale(UVec2::new(300, 200)), 1);
        assert_eq!(GuiSize::of(UVec2::new(1920, 1080), 0).scale, 4);
        assert_eq!(
            GuiSize::of(UVec2::new(854, 480), 3),
            GuiSize {
                width: 427,
                height: 240,
                scale: 2
            }
        );
    }

    mod jar {
        use super::*;

        #[test]
        fn the_gui_atlas_holds_every_region_and_the_digit_widths() {
            let atlas = GuiAtlasData::load(ResourceSet::corpus()).unwrap();
            for name in GUI_TEXTURES
                .iter()
                .filter(|n| **n != "misc/enchanted_glint_item")
            {
                assert!(atlas.regions.contains_key(name), "{name}");
            }
            assert_eq!(atlas.region("hud/hotbar").size(), IVec2::new(182, 22));
            assert_eq!(
                atlas.region("container/inventory").size(),
                IVec2::new(176, 166)
            );
            assert_eq!(
                atlas.region("container/gamemode_switcher").size(),
                IVec2::new(125, 75)
            );
            assert_eq!(
                atlas.region("gamemode_switcher/slot").size(),
                IVec2::new(26, 26)
            );
            assert_eq!(atlas.font.advance('7', false), 6.0);
            assert_eq!(atlas.glint.0, 128);
            assert!(atlas.height <= 2048);
            let blank = atlas.region(BLANK);
            let start = (blank.min.y as usize * atlas.width as usize + blank.min.x as usize) * 4;
            assert_eq!(&atlas.pixels[start..start + 4], &[255, 255, 255, 255]);
        }

        #[test]
        fn the_button_is_a_nine_slice_and_a_sprite_the_pack_lacks_is_the_missing_sprite() {
            let atlas = GuiAtlasData::load(ResourceSet::corpus()).unwrap();
            for id in GuiSpriteId::ALL {
                assert_ne!(atlas.sprite(id), atlas.missing, "{}", id.name());
            }
            for id in GuiTextureId::ALL {
                assert!(atlas.regions.contains_key(id.name()), "{}", id.name());
            }
            let button = atlas.region(GuiSpriteId::Button.name()).min;
            let mut out = Vec::new();
            atlas
                .sprite(GuiSpriteId::Button)
                .draw(IRect::new(0, 0, 50, 20), &mut out);
            assert_eq!(out.len(), 9, "{out:?}");
            for (at, texel) in [((0, 0), (0, 0)), ((47, 0), (197, 0)), ((0, 17), (0, 17))] {
                let (at, texel) = (IVec2::from(at), button + IVec2::from(texel));
                let corner = GuiQuad::Texels {
                    rect: IRect::from_corners(at, at + 3),
                    texels: Rect::from_corners(texel.as_vec2(), (texel + 3).as_vec2()),
                };
                assert!(out.contains(&corner), "{corner:?} in {out:?}");
            }

            let gui = [
                "minecraft/font/",
                "minecraft/textures/font/",
                "minecraft/textures/gui/",
                "minecraft/textures/misc/",
            ];
            let without_button: ResourceSet =
                mcrs_minecraft_model::vanilla_source::resource_files()
                    .iter()
                    .filter(|(path, _)| gui.iter().any(|folder| path.starts_with(folder)))
                    .filter(|(path, _)| !path.contains("sprites/widget/button.png"))
                    .cloned()
                    .collect();
            let atlas = GuiAtlasData::load(&without_button).unwrap();
            assert_eq!(atlas.sprite(GuiSpriteId::Button), atlas.missing);
            assert_ne!(atlas.sprite(GuiSpriteId::ButtonHighlighted), atlas.missing);
        }
    }
}
