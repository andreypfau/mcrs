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

use crate::font::Font;
use crate::hotbar::hotbar;
use crate::inventory_screen::inventory_screen;
use crate::item_decorations::{Decorated, WHITE, decorations};
use crate::item_resolve::{GuiVertex, ItemRenderLayers};
use crate::language::ClientLanguage;
use crate::sprite::{GuiSprite, Scaling};
use crate::tooltip::{Tooltip, TooltipSprites};
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

    pub fn of(framebuffer: UVec2, scale: u32) -> Self {
        let scale = if scale == 0 {
            Self::auto_scale(framebuffer)
        } else {
            scale
        };
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
    Glyph {
        origin: IVec2,
        glyph: char,
        color: u32,
    },
    Item {
        origin: IVec2,
        stack: Entity,
    },
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

const TOOLTIP_SPRITES: [&str; 2] = ["tooltip/background", "tooltip/frame"];

const GUI_TEXTURES: [&str; 19] = [
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
    "font/ascii",
    "misc/enchanted_glint_item",
    "icon/chat_modified",
];

fn texture_path(name: &str) -> String {
    match name {
        "container/inventory" | "container/gamemode_switcher" => {
            format!("minecraft/textures/gui/{name}.png")
        }
        "font/ascii" | "misc/enchanted_glint_item" => format!("minecraft/textures/{name}.png"),
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
    pub tooltip: TooltipSprites,
    pub missing: GuiSprite,
    pub ascii_origin: IVec2,
    pub chat_modified: IRect,
}

#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GuiAtlas(pub Arc<GuiAtlasData>);

impl GuiAtlasData {
    /// Shelf-packs the GUI textures the screens blit, which are not square and so
    /// bypass the sprite arrays; the glint stays separate because it repeats.
    pub fn load(pack: &ResourceSet) -> Result<Self, String> {
        let mut images = Vec::new();
        let mut glint = None;
        let mut font = None;
        let tooltip_scaling = load_tooltip_sprites(pack, &mut images);
        for name in GUI_TEXTURES {
            let path = texture_path(name);
            let (pixels, width, height) =
                decode_png(pack.read(&path).map_err(|error| error.to_string())?, &path)
                    .map_err(|error| error.to_string())?;
            match name {
                "misc/enchanted_glint_item" => glint = Some((width, height, pixels)),
                "container/inventory" => {
                    images.push((name, crop(&pixels, width, 176, 166), 176, 166))
                }
                "container/gamemode_switcher" => images.push((
                    name,
                    crop(&pixels, width, SWITCHER_WIDTH, SWITCHER_HEIGHT),
                    SWITCHER_WIDTH,
                    SWITCHER_HEIGHT,
                )),
                _ => {
                    if name == "font/ascii" {
                        font = Some(Font::load(pack, &pixels, width, height)?);
                    }
                    images.push((name, pixels, width, height));
                }
            }
        }
        images.push((BLANK, vec![255; 16], 2, 2));
        images.push((MISSING, missing_pixels(), MISSING_SIZE, MISSING_SIZE));
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
        let atlas_height = (y + row).next_power_of_two();
        let mut pixels = vec![0u8; (ATLAS_WIDTH * atlas_height * 4) as usize];
        let mut regions = HashMap::new();
        for (name, source, rect) in placed {
            let width = rect.width() as usize;
            for (line, source_row) in source.chunks_exact(width * 4).enumerate() {
                let start =
                    ((rect.min.y as usize + line) * ATLAS_WIDTH as usize + rect.min.x as usize) * 4;
                if let Some(target) = pixels.get_mut(start..start + width * 4) {
                    target.copy_from_slice(source_row);
                }
            }
            regions.insert(name, rect);
        }
        let [background, frame] = TOOLTIP_SPRITES.map(|name| regions.get(name).copied());
        let [background_scaling, frame_scaling] = tooltip_scaling;
        let tooltip = TooltipSprites {
            background: background
                .zip(background_scaling)
                .map(|(rect, s)| GuiSprite::new(rect, s)),
            frame: frame
                .zip(frame_scaling)
                .map(|(rect, s)| GuiSprite::new(rect, s)),
        };
        let missing = GuiSprite::new(
            regions.get(MISSING).copied().unwrap_or_default(),
            Scaling::Stretch,
        );
        let ascii_origin = regions.get("font/ascii").copied().unwrap_or_default().min;
        let chat_modified = regions
            .get("icon/chat_modified")
            .copied()
            .unwrap_or_default();
        Ok(Self {
            width: ATLAS_WIDTH,
            height: atlas_height,
            pixels,
            regions,
            font: font.ok_or("the GUI atlas lists no font/ascii")?,
            glint: glint.ok_or("the GUI atlas lists no glint")?,
            tooltip,
            missing,
            ascii_origin,
            chat_modified,
        })
    }

    pub fn region(&self, name: &str) -> IRect {
        self.regions.get(name).copied().unwrap_or_default()
    }

    fn uv(&self, pixel: IRect) -> [[f32; 2]; 4] {
        self.texel_uv(pixel.as_rect())
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

/// The scaling of the background and the outline sprites of a tooltip. A sprite the pack lacks,
/// or one with an error, is left out, so it draws the missing sprite.
fn load_tooltip_sprites(
    pack: &ResourceSet,
    images: &mut Vec<(&'static str, Vec<u8>, u32, u32)>,
) -> [Option<Scaling>; 2] {
    TOOLTIP_SPRITES.map(|name| {
        let path = texture_path(name);
        let bytes = pack.get(&path)?;
        let loaded = Scaling::read(pack.get(&format!("{path}.mcmeta"))).and_then(|scaling| {
            let image = decode_png(bytes, &path).map_err(|error| error.to_string())?;
            Ok((scaling, image))
        });
        match loaded {
            Ok((scaling, (pixels, width, height))) => {
                images.push((name, pixels, width, height));
                Some(scaling)
            }
            Err(reason) => {
                error!("GUI sprite minecraft:{name}: {reason}");
                None
            }
        }
    })
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
    pub item: bool,
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

fn push_quad(
    vertices: &mut Vec<GuiVertex>,
    rect: IRect,
    uv: [[f32; 2]; 4],
    colors: [u32; 4],
    sprite: u32,
) {
    let corners = [
        [rect.min.x as f32, rect.min.y as f32, 0.0],
        [rect.max.x as f32, rect.min.y as f32, 0.0],
        [rect.max.x as f32, rect.max.y as f32, 0.0],
        [rect.min.x as f32, rect.max.y as f32, 0.0],
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

fn begin_gui_frame(
    window: Single<&Window, With<PrimaryWindow>>,
    config: Res<GuiConfig>,
    mut scene: ResMut<GuiScene>,
) {
    let framebuffer = UVec2::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
    );
    scene.size = GuiSize::of(framebuffer, config.scale);
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

fn build_gui_batch(
    scene: Res<GuiScene>,
    atlas: Option<Res<GuiAtlas>>,
    layers: Query<&ItemRenderLayers>,
    stacks: Query<EntityRef, (With<ItemStack>, Without<IsResource>)>,
    time: Res<Time<Real>>,
    config: Res<GuiConfig>,
    mut batch: ResMut<GuiBatch>,
) {
    let batch = &mut *batch;
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
                item: false,
            });
        }
    };
    for quad in expanded {
        let vertices = &mut batch.vertices;
        if let GuiQuad::Item { origin, stack } = quad {
            let end = vertices.len() as u32;
            close_flat(&mut flat_start, end, &mut batch.draws);
            let Ok(layers) = layers.get(stack) else {
                continue;
            };
            push_item_layers(vertices, layers, origin);
            let after = vertices.len() as u32;
            if after != end {
                batch.draws.push(GuiDraw {
                    range: end..after,
                    item: true,
                });
            }
        } else {
            if flat_start.is_none() {
                flat_start = Some(vertices.len() as u32);
            }
            push_flat_quad(vertices, atlas, blank, quad);
        }
    }
    let end = batch.vertices.len() as u32;
    close_flat(&mut flat_start, end, &mut batch.draws);
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

/// The vertices of a quad drawn from the GUI atlas; an item quad draws its own layers instead.
fn push_flat_quad(
    vertices: &mut Vec<GuiVertex>,
    atlas: &GuiAtlasData,
    blank: [[f32; 2]; 4],
    quad: GuiQuad,
) {
    match quad {
        GuiQuad::Sprite {
            rect,
            region,
            color,
        } => {
            push_quad(
                vertices,
                rect,
                atlas.uv(atlas.region(region)),
                [color; 4],
                GUI_ATLAS_BIT,
            );
        }
        GuiQuad::Fill { rect, color } => {
            push_quad(vertices, rect, blank, [color; 4], GUI_ATLAS_BIT)
        }
        GuiQuad::Texels { rect, texels } => push_quad(
            vertices,
            rect,
            atlas.texel_uv(texels),
            [WHITE; 4],
            GUI_ATLAS_BIT,
        ),
        GuiQuad::FillGradient { rect, top, bottom } => {
            push_quad(
                vertices,
                rect,
                blank,
                [top, top, bottom, bottom],
                GUI_ATLAS_BIT,
            );
        }
        GuiQuad::Glyph {
            origin,
            glyph,
            color,
        } => {
            let Some(glyph) = atlas.font.glyph(glyph) else {
                return;
            };
            let ascii = atlas.ascii_origin + glyph.cell;
            let cell = atlas.font.cell_size;
            let pixel = IRect::from_corners(ascii, ascii + cell);
            let rect = IRect::from_corners(origin, origin + cell);
            push_quad(vertices, rect, atlas.uv(pixel), [color; 4], GUI_ATLAS_BIT);
        }
        GuiQuad::Item { .. } => {}
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
            .add_plugins(ExtractResourcePlugin::<GuiBatch>::default())
            .add_plugins(ExtractResourcePlugin::<GuiAtlas>::default())
            .add_systems(
                PostUpdate,
                (
                    begin_gui_frame,
                    draw_screens,
                    crate::chat::draw,
                    crate::game_mode_switcher::draw,
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
            assert_eq!(atlas.font.advance('7', false), 6);
            assert_eq!(atlas.glint.0, 128);
            assert!(atlas.height <= 512);
            let blank = atlas.region(BLANK);
            let start = (blank.min.y as usize * atlas.width as usize + blank.min.x as usize) * 4;
            assert_eq!(&atlas.pixels[start..start + 4], &[255, 255, 255, 255]);
        }
    }
}
