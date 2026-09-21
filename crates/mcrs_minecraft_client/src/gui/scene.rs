use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::resource::IsResource;
use bevy::ecs::schedule::SystemSet;
use bevy::ecs::world::EntityRef;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::math::{IRect, IVec2, UVec2};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_item::{
    ItemStack, SelectedHotbarSlot, SlotTable, damage_value, is_damaged, max_damage,
};

use super::hotbar::hotbar;
use super::inventory_screen::inventory_screen;
use super::item_decorations::{Decorated, decorations};
use crate::inventory::Screen;
use crate::item_model::resolve::{Foil, GuiVertex, ItemRenderLayers};
use crate::model::Pack;
use crate::player::Player;
use crate::stream::BlockCatalog;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuiSize {
    pub width: i32,
    pub height: i32,
    pub scale: u32,
}

impl GuiSize {
    pub fn auto_scale(framebuffer: UVec2) -> u32 {
        let mut scale = 1;
        while framebuffer.x / (scale + 1) >= 320 && framebuffer.y / (scale + 1) >= 240 {
            scale += 1;
        }
        scale
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
    Glyph {
        origin: IVec2,
        digit: u8,
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
}

#[derive(Resource)]
pub struct GuiConfig {
    pub scale: u32,
    pub cursor: Option<IVec2>,
    pub frozen_ticks: Option<i64>,
}

const GLYPH_CELL: i32 = 8;
const DIGIT_ROW: i32 = 3;
const ATLAS_WIDTH: u32 = 512;
const BLANK: &str = "blank";

const GUI_TEXTURES: [(&str, &str); 16] = [
    (
        "hud/hotbar",
        "minecraft/textures/gui/sprites/hud/hotbar.png",
    ),
    (
        "hud/hotbar_selection",
        "minecraft/textures/gui/sprites/hud/hotbar_selection.png",
    ),
    (
        "hud/hotbar_offhand_left",
        "minecraft/textures/gui/sprites/hud/hotbar_offhand_left.png",
    ),
    (
        "hud/hotbar_offhand_right",
        "minecraft/textures/gui/sprites/hud/hotbar_offhand_right.png",
    ),
    (
        "container/slot_highlight_back",
        "minecraft/textures/gui/sprites/container/slot_highlight_back.png",
    ),
    (
        "container/slot_highlight_front",
        "minecraft/textures/gui/sprites/container/slot_highlight_front.png",
    ),
    (
        "container/slot/helmet",
        "minecraft/textures/gui/sprites/container/slot/helmet.png",
    ),
    (
        "container/slot/chestplate",
        "minecraft/textures/gui/sprites/container/slot/chestplate.png",
    ),
    (
        "container/slot/leggings",
        "minecraft/textures/gui/sprites/container/slot/leggings.png",
    ),
    (
        "container/slot/boots",
        "minecraft/textures/gui/sprites/container/slot/boots.png",
    ),
    (
        "container/slot/shield",
        "minecraft/textures/gui/sprites/container/slot/shield.png",
    ),
    (
        "recipe_book/button",
        "minecraft/textures/gui/sprites/recipe_book/button.png",
    ),
    (
        "recipe_book/button_highlighted",
        "minecraft/textures/gui/sprites/recipe_book/button_highlighted.png",
    ),
    (
        "container/inventory",
        "minecraft/textures/gui/container/inventory.png",
    ),
    ("font/ascii", "minecraft/textures/font/ascii.png"),
    (
        "misc/enchanted_glint_item",
        "minecraft/textures/misc/enchanted_glint_item.png",
    ),
];

pub struct GuiAtlasData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub regions: HashMap<&'static str, IRect>,
    pub digit_widths: [u8; 10],
    pub glint: (u32, u32, Vec<u8>),
}

#[derive(Resource, Clone, ExtractResource)]
pub struct GuiAtlas(pub Arc<GuiAtlasData>);

fn decode_png(bytes: &[u8], path: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .map_err(|error| format!("cannot decode {path}: {error}"))?;
    let (width, height) = (image.width(), image.height());
    let data = image
        .data
        .ok_or_else(|| format!("{path} decoded without pixel data"))?;
    Ok((data, width, height))
}

fn digit_widths(ascii: &[u8], width: u32) -> [u8; 10] {
    let mut widths = [0u8; 10];
    for (digit, out) in widths.iter_mut().enumerate() {
        let cell_x = digit as u32 * GLYPH_CELL as u32;
        let cell_y = DIGIT_ROW as u32 * GLYPH_CELL as u32;
        let column = (0..GLYPH_CELL as u32).rev().find(|&x| {
            (0..GLYPH_CELL as u32)
                .any(|y| ascii[((cell_y + y) * width + cell_x + x) as usize * 4 + 3] != 0)
        });
        *out = column.map_or(0, |x| x as u8 + 1);
    }
    widths
}

impl GuiAtlasData {
    /// Shelf-packs the GUI textures the screens blit, which are not square and so
    /// bypass the sprite arrays; the glint stays separate because it repeats.
    pub fn load(pack: &Pack) -> Result<Self, String> {
        let mut images = Vec::new();
        let mut glint = None;
        let mut digits = None;
        for (name, path) in GUI_TEXTURES {
            let (pixels, width, height) = decode_png(pack.read(path)?, path)?;
            match name {
                "misc/enchanted_glint_item" => glint = Some((width, height, pixels)),
                "container/inventory" => {
                    images.push((name, crop(&pixels, width, 176, 166), 176, 166))
                }
                _ => {
                    if name == "font/ascii" {
                        digits = Some(digit_widths(&pixels, width));
                    }
                    images.push((name, pixels, width, height));
                }
            }
        }
        images.push((BLANK, vec![255; 16], 2, 2));
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
                pixels[start..start + width * 4].copy_from_slice(source_row);
            }
            regions.insert(name, rect);
        }
        Ok(Self {
            width: ATLAS_WIDTH,
            height: atlas_height,
            pixels,
            regions,
            digit_widths: digits.expect("font/ascii is listed"),
            glint: glint.expect("the glint is listed"),
        })
    }

    pub fn region(&self, name: &str) -> IRect {
        self.regions[name]
    }

    fn uv(&self, pixel: IRect) -> [[f32; 2]; 4] {
        let (w, h) = (self.width as f32, self.height as f32);
        let (x0, y0, x1, y1) = (
            pixel.min.x as f32 / w,
            pixel.min.y as f32 / h,
            pixel.max.x as f32 / w,
            pixel.max.y as f32 / h,
        );
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }
}

fn crop(pixels: &[u8], stride: u32, width: u32, height: u32) -> Vec<u8> {
    let stride = stride as usize * 4;
    let width = width as usize * 4;
    (0..height as usize)
        .flat_map(|row| pixels[row * stride..row * stride + width].iter().copied())
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

fn rgba(argb: u32) -> [u8; 4] {
    let [a, r, g, b] = argb.to_be_bytes();
    [r, g, b, a]
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
    let quad: [GuiVertex; 4] = std::array::from_fn(|i| GuiVertex {
        pos: corners[i],
        uv: uv[i],
        color: rgba(colors[i]),
        sprite,
    });
    vertices.extend([quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuiSet {
    Begin,
    Screens,
    Build,
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

fn load_gui_atlas(catalog: Res<BlockCatalog>, mut commands: Commands) {
    let Some(pack) = catalog.pack() else {
        return;
    };
    let atlas = GuiAtlasData::load(pack)
        .unwrap_or_else(|reason| panic!("cannot load the GUI textures: {reason}"));
    commands.insert_resource(GuiAtlas(Arc::new(atlas)));
}

#[allow(clippy::too_many_arguments)]
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
                    count: stack.get::<ItemStack>().map_or(1, ItemStack::count),
                    damage: is_damaged(stack).then(|| (damage_value(stack), max_damage(stack))),
                    cooldown: 0.0,
                };
                decorations(origin, &decorated, &atlas.digit_widths, &mut expanded);
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
        match quad {
            GuiQuad::Item { origin, stack } => {
                let end = vertices.len() as u32;
                close_flat(&mut flat_start, end, &mut batch.draws);
                let Ok(layers) = layers.get(stack) else {
                    continue;
                };
                for layer in &layers.layers {
                    let glint = if layer.foil == Foil::None {
                        0
                    } else {
                        GLINT_BIT
                    };
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
                        vertices.extend([0, 1, 2, 0, 2, 3].map(|i| shifted(&quad[i])));
                    }
                }
                let after = vertices.len() as u32;
                if after != end {
                    batch.draws.push(GuiDraw {
                        range: end..after,
                        item: true,
                    });
                }
            }
            flat => {
                if flat_start.is_none() {
                    flat_start = Some(vertices.len() as u32);
                }
                match flat {
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
                        digit,
                        color,
                    } => {
                        let ascii = atlas.region("font/ascii");
                        let cell = IVec2::new(digit as i32 * GLYPH_CELL, DIGIT_ROW * GLYPH_CELL);
                        let pixel =
                            IRect::from_corners(ascii.min + cell, ascii.min + cell + GLYPH_CELL);
                        let rect = IRect::from_corners(origin, origin + GLYPH_CELL);
                        push_quad(vertices, rect, atlas.uv(pixel), [color; 4], GUI_ATLAS_BIT);
                    }
                    GuiQuad::Item { .. } => unreachable!(),
                }
            }
        }
    }
    let end = batch.vertices.len() as u32;
    close_flat(&mut flat_start, end, &mut batch.draws);
}

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        let config = GuiConfig {
            scale: crate::config::gui_scale(),
            cursor: crate::config::gui_cursor(),
            frozen_ticks: crate::config::frozen_time(),
        };
        app.insert_resource(config)
            .insert_resource(crate::config::initial_screen())
            .init_resource::<GuiScene>()
            .init_resource::<GuiBatch>()
            .add_plugins(ExtractResourcePlugin::<GuiBatch>::default())
            .add_plugins(ExtractResourcePlugin::<GuiAtlas>::default())
            .configure_sets(
                PostUpdate,
                (GuiSet::Begin, GuiSet::Screens, GuiSet::Build).chain(),
            )
            .add_systems(
                Update,
                load_gui_atlas.run_if(not(resource_exists::<GuiAtlas>)),
            )
            .add_systems(
                PostUpdate,
                (
                    begin_gui_frame.in_set(GuiSet::Begin),
                    draw_screens.in_set(GuiSet::Screens),
                    build_gui_batch.in_set(GuiSet::Build),
                ),
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

    #[test]
    fn the_gui_atlas_holds_every_region_and_the_digit_widths() {
        let atlas = GuiAtlasData::load(Pack::corpus()).unwrap();
        for (name, _) in GUI_TEXTURES
            .iter()
            .filter(|(n, _)| *n != "misc/enchanted_glint_item")
        {
            assert!(atlas.regions.contains_key(name), "{name}");
        }
        assert_eq!(atlas.region("hud/hotbar").size(), IVec2::new(182, 22));
        assert_eq!(
            atlas.region("container/inventory").size(),
            IVec2::new(176, 166)
        );
        assert_eq!(atlas.digit_widths, [5; 10]);
        assert_eq!(atlas.glint.0, 128);
        assert!(atlas.height <= 512);
        let blank = atlas.region(BLANK);
        let start = (blank.min.y as usize * atlas.width as usize + blank.min.x as usize) * 4;
        assert_eq!(&atlas.pixels[start..start + 4], &[255, 255, 255, 255]);
    }

    #[test]
    fn glint_offsets_wrap_like_vanilla() {
        assert_eq!(glint_offset(0), [0.0, 0.0]);
        let [o0, o1] = glint_offset(1000);
        assert!((o0 - 4000.0 / 110_000.0).abs() < 1e-6);
        assert!((o1 - 4000.0 / 30_000.0).abs() < 1e-6);
    }
}
