use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use bevy::math::{DVec2, IRect, IVec2, Rect, UVec2};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_geometry::MenuGeometry;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::atlas_quad::AtlasQuad;
use crate::creative_item_tooltip::request_creative_item_tooltip;
use crate::creative_screen_draw::{creative_sprites, draw_creative_screen};
use crate::creative_screen_shown::{creative_screen_shown, shows_creative_screen};
use crate::drag_preview::DragPreview;
use crate::font::Font;
use crate::font_definition::FontOptions;
use crate::font_glyph::{
    GlyphImage, ImageSource, MISSING_GLYPH_SIZE, Weight, missing_glyph_pixels,
};
use crate::glyph_cache::{self, GlyphCache};
use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;
use crate::hotbar::hotbar;
use crate::inventory_figure::{pose_inventory_figure, spawn_inventory_figure};
use crate::item_decorations::{CountLabel, Decorated, WHITE, decorations};
use crate::item_icons::{ItemIcons, build_icon_meshes};
use crate::language::ClientLanguage;
use crate::menu_draw;
use crate::menu_label_text::MenuLabelText;
use crate::menu_screen::{
    MenuContents, background_extent, background_region, menu_backgrounds, menu_screen,
};
use crate::menu_stacks::MenuStacks;
use crate::slot_icon::SlotIcon;
use crate::sprite::{GuiSprite, Scaling};
use crate::tooltip::Tooltip;
use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_entity_render::{EntitySlotSystems, FigureView};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item_icon::{
    GuiVertex, IconLayer, IconMeshes, ItemIcon, Overlays, StackOverlay,
};
use mcrs_minecraft_model::atlas::{MISSING_SIZE, decode_png, missing_pixels, rgba};
use mcrs_minecraft_model::item_model_bake::ItemModels;
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

#[derive(Resource, Default)]
pub struct GuiScene {
    pub size: GuiSize,
    pub cursor: IVec2,
    pub quads: Vec<GuiQuad>,
    pub icons: Vec<SceneIcon>,
    pub tooltip: Tooltip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneIcon {
    pub icon: ItemIcon,
    pub overlay: StackOverlay,
    pub overlays: Overlays,
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
const GLYPH_CACHE: &str = "glyph cache";
const CELL: usize = crate::font_unihex::CELL;

const GUI_TEXTURES: [&str; 17] = [
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
    "container/gamemode_switcher",
    "gamemode_switcher/slot",
    "gamemode_switcher/selection",
    "misc/enchanted_glint_item",
    "icon/chat_modified",
];

fn texture_path(name: &str) -> String {
    match name {
        "container/gamemode_switcher" => {
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
    pub page_origins: Vec<(usize, IVec2)>,
    pub missing_glyph: IRect,
    pub glyph_origin: IVec2,
    pub chat_modified: IRect,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Packed {
    Named(&'static str),
    Page(usize),
}

#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GuiAtlas(pub Arc<GuiAtlasData>);

impl GuiAtlasData {
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
                "container/gamemode_switcher" => images.push((
                    Packed::Named(name),
                    crop(&pixels, width, SWITCHER_WIDTH, SWITCHER_HEIGHT),
                    SWITCHER_WIDTH,
                    SWITCHER_HEIGHT,
                )),
                _ => images.push((Packed::Named(name), pixels, width, height)),
            }
        }
        for name in creative_sprites() {
            load_image(
                pack,
                name,
                &texture_path(name),
                Ok(Scaling::Stretch),
                &mut images,
            );
        }
        let mut backgrounds = Vec::new();
        for texture in menu_backgrounds() {
            let Some(name) = background_region(texture) else {
                continue;
            };
            if backgrounds.contains(&name) {
                continue;
            }
            backgrounds.push(name);
            let path = format!("{}/{}", texture.namespace(), texture.path());
            let (pixels, width, height) =
                decode_png(pack.read(&path).map_err(|error| error.to_string())?, &path)
                    .map_err(|error| error.to_string())?;
            let extent = background_extent(texture).min(UVec2::new(width, height));
            images.push((
                Packed::Named(name),
                crop(&pixels, width, extent.x, extent.y),
                extent.x,
                extent.y,
            ));
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
        let (cache_width, cache_height) = (
            (glyph_cache::COLUMNS * CELL) as u32,
            (glyph_cache::ROWS * CELL) as u32,
        );
        images.push((
            Packed::Named(GLYPH_CACHE),
            vec![0; (cache_width * cache_height * 4) as usize],
            cache_width,
            cache_height,
        ));
        images.sort_by_key(|(name, _, _, height)| (std::cmp::Reverse(*height), *name));
        let atlas_width = images
            .iter()
            .map(|(_, _, width, _)| *width)
            .fold(ATLAS_WIDTH, u32::max);
        let (mut x, mut y, mut row) = (0u32, 0u32, 0u32);
        let mut placed = Vec::new();
        for (name, pixels, width, height) in images {
            if x + width > atlas_width {
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
        let mut pixels = vec![0u8; (atlas_width * atlas_height * 4) as usize];
        let mut regions = HashMap::new();
        let mut page_origins = Vec::new();
        for (name, source, rect) in placed {
            let width = rect.width() as usize;
            for (line, source_row) in source.chunks_exact(width * 4).enumerate() {
                let start =
                    ((rect.min.y as usize + line) * atlas_width as usize + rect.min.x as usize) * 4;
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
        let glyph_origin = regions
            .get(GLYPH_CACHE)
            .map_or(IVec2::ZERO, |region| region.min);
        let chat_modified = regions
            .get("icon/chat_modified")
            .copied()
            .unwrap_or_default();
        Ok(Self {
            width: atlas_width,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiDrawKind {
    Flat,
    Item,
    InvertRedGreen,
    FullBlue,
    Figure { rect: IRect },
}

#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GuiBatch {
    pub vertices: Vec<GuiVertex>,
    pub draws: Vec<GuiDraw>,
    pub scale: u32,
    pub glint_offset: [f32; 2],
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

pub(crate) fn window_framebuffer(window: &Window) -> UVec2 {
    UVec2::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
    )
}

pub(crate) fn gui_pointer(window: &Window, config: &GuiConfig, scale: u32) -> Option<DVec2> {
    if let Some(cursor) = config.cursor {
        return Some(cursor.as_dvec2());
    }
    let scale = f64::from(scale.max(1));
    window
        .physical_cursor_position()
        .map(|cursor| cursor.as_dvec2() / scale)
}

pub(crate) fn gui_point(at: DVec2) -> GuiPoint {
    GuiPoint::new(at.x.floor() as i32, at.y.floor() as i32)
}

fn begin_gui_frame(
    window: Single<&Window, With<PrimaryWindow>>,
    config: Res<GuiConfig>,
    mut scene: ResMut<GuiScene>,
) {
    scene.size = GuiSize::of(window_framebuffer(&window), config.scale);
    scene.cursor = gui_pointer(&window, &config, scene.size.scale)
        .map(gui_point)
        .map_or(IVec2::new(-1, -1), menu_draw::point);
    scene.quads.clear();
    scene.icons.clear();
    scene.tooltip.clear();
}

fn draw_tooltip(atlas: Option<Res<GuiAtlas>>, mut scene: ResMut<GuiScene>) {
    let Some(atlas) = atlas else {
        return;
    };
    let scene = &mut *scene;
    crate::tooltip::draw(&scene.tooltip, scene.size, &atlas.0, &mut scene.quads);
}

#[derive(Default)]
pub(crate) struct ScreenStacks {
    inventory: MenuStacks,
    open: MenuStacks,
    labels: MenuLabelText,
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each access is its own parameter"
)]
pub(crate) fn draw_screens(
    screen: Res<Screen>,
    player: Query<(
        &PlayerHolders,
        &Menus,
        &Selected,
        Option<&PlayerGameMode>,
        Option<&Abilities>,
    )>,
    menus: Query<(&MenuLayout, &MenuParts, Option<Ref<MenuTitle>>)>,
    holders: Query<&Slots>,
    atlas: Option<Res<GuiAtlas>>,
    language: Option<Res<ClientLanguage>>,
    drag: Option<Res<DragPreview>>,
    icons: Query<&SlotIcon>,
    mut stacks: Local<ScreenStacks>,
    mut scene: ResMut<GuiScene>,
) {
    let scene = &mut *scene;
    let stacks = &mut *stacks;
    let Ok((player, player_menus, selected, mode, abilities)) = player.single() else {
        return;
    };
    let occupant = |slot: SlotRef| holders.get(slot.holder).ok()?.get(usize::from(slot.index));
    let icon_of = |stack| icons.get(stack).ok().copied();
    if mode.is_none_or(|PlayerGameMode(mode)| *mode != GameMode::Spectator)
        && let Some((layout, parts, _)) =
            (player_menus.iter().next()).and_then(|menu| menus.get(menu).ok())
    {
        stacks
            .inventory
            .collect(layout, parts, player.cursor, occupant);
        hotbar(scene, selected.0, &stacks.inventory, icon_of);
    }
    let Some(open) = player_menus.open() else {
        return;
    };
    let Ok((layout, parts, title)) = menus.get(open) else {
        return;
    };
    if let Some(language) = language {
        stacks.labels.follow(open, title, language.into());
    }
    if *screen != Screen::Menu || shows_creative_screen(*screen, player_menus, abilities) {
        return;
    }
    let (Some(atlas), Some(geometry)) = (atlas, MenuGeometry::of_layout(layout)) else {
        return;
    };
    stacks.open.collect(layout, parts, player.cursor, occupant);
    let no_drag = DragPreview::default();
    menu_screen(
        scene,
        geometry,
        MenuContents {
            stacks: &stacks.open,
            drag: drag.as_deref().unwrap_or(&no_drag),
        },
        &stacks.labels,
        &atlas.0,
        icon_of,
    );
}

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
    reason = "a Bevy system: each access is its own parameter"
)]
fn build_gui_batch(
    scene: Res<GuiScene>,
    atlas: Option<Res<GuiAtlas>>,
    icons: ItemIcons,
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
            GuiQuad::Icon { origin, index } => {
                expanded.push(*quad);
                let Some(entry) = scene.icons.get(index) else {
                    continue;
                };
                let count = match entry.overlays {
                    Overlays::Stack => CountLabel::Plain(entry.overlay.count),
                    Overlays::Capped => CountLabel::Capped(entry.overlay.count),
                    _ => continue,
                };
                let decorated = Decorated {
                    count,
                    damage: entry.overlay.durability,
                };
                decorations(origin, &decorated, &atlas.font, &mut expanded);
            }
            other => expanded.push(other),
        }
    }
    let mut flat_start = None;
    for quad in expanded {
        let vertices = &mut batch.vertices;
        match quad {
            GuiQuad::Icon { origin, index } => {
                let end = vertices.len() as u32;
                close_flat(&mut flat_start, end, &mut batch.draws);
                let (Some(entry), Some(meshes)) = (scene.icons.get(index), icons.meshes()) else {
                    continue;
                };
                push_icon(vertices, &entry.icon, meshes, origin);
                push_item_draw(end, vertices, &mut batch.draws);
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
            GuiQuad::Figure { rect } => {
                let rect = clips.last().map_or(rect, |clip| clip.intersect(rect));
                if rect.is_empty() {
                    continue;
                }
                let end = vertices.len() as u32;
                close_flat(&mut flat_start, end, &mut batch.draws);
                batch.draws.push(GuiDraw {
                    range: end..end,
                    kind: GuiDrawKind::Figure { rect },
                });
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

fn close_flat(start: &mut Option<u32>, end: u32, draws: &mut Vec<GuiDraw>) {
    if let Some(begin) = start.take()
        && begin != end
    {
        draws.push(GuiDraw {
            range: begin..end,
            kind: GuiDrawKind::Flat,
        });
    }
}

fn push_item_draw(start: u32, vertices: &[GuiVertex], draws: &mut Vec<GuiDraw>) {
    let after = vertices.len() as u32;
    if after != start {
        draws.push(GuiDraw {
            range: start..after,
            kind: GuiDrawKind::Item,
        });
    }
}

fn push_icon(vertices: &mut Vec<GuiVertex>, icon: &ItemIcon, meshes: &IconMeshes, origin: IVec2) {
    let glint = if icon.foil() { GLINT_BIT } else { 0 };
    for IconLayer { model, tints } in icon.layers() {
        for quad in meshes.get(*model).into_iter().flatten() {
            push_item_quad(vertices, &quad.tinted(tints), origin, glint);
        }
    }
}

fn push_item_quad(vertices: &mut Vec<GuiVertex>, quad: &[GuiVertex; 4], origin: IVec2, glint: u32) {
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
        GuiQuad::Texels {
            rect,
            texels,
            color,
        } => AtlasQuad {
            rect: rect.as_rect(),
            uv: atlas.texel_uv(texels),
            colors: [color; 4],
        },
        GuiQuad::FillGradient { rect, top, bottom } => AtlasQuad {
            rect: rect.as_rect(),
            uv: blank,
            colors: [top, top, bottom, bottom],
        },
        GuiQuad::Glyph {
            pen,
            glyph,
            image,
            color,
            weight,
        } => {
            let Some(texels) = atlas.glyph_texels(image.source, glyph, cache) else {
                return;
            };
            AtlasQuad {
                rect: image.strike(pen, weight),
                uv: atlas.texel_uv(texels),
                colors: [color; 4],
            }
        }
        GuiQuad::Icon { .. }
        | GuiQuad::Highlight { .. }
        | GuiQuad::Clip { .. }
        | GuiQuad::EndClip
        | GuiQuad::Figure { .. } => return,
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
            .add_plugins(crate::creative_screen_plugin::CreativeScreenPlugin)
            .add_observer(spawn_inventory_figure)
            .init_resource::<crate::title_draw::TitleScreenState>()
            .add_plugins(ExtractResourcePlugin::<GuiBatch>::default())
            .add_plugins(ExtractResourcePlugin::<GuiAtlas>::default())
            .add_systems(
                Update,
                build_icon_meshes.run_if(resource_exists_and_changed::<ItemModels>),
            )
            .add_systems(
                PostUpdate,
                (
                    begin_gui_frame,
                    draw_screens,
                    crate::menu_item_tooltip::request_menu_item_tooltip
                        .run_if(resource_exists::<ItemTable>)
                        .run_if(not(creative_screen_shown)),
                    draw_creative_screen.run_if(creative_screen_shown),
                    request_creative_item_tooltip
                        .run_if(resource_exists::<ItemTable>)
                        .run_if(creative_screen_shown),
                    crate::chat::draw,
                    crate::game_mode_switcher::draw,
                    crate::menu_draw::draw.run_if(resource_exists::<crate::menu::Menu>),
                    crate::message_screen::draw.run_if(|screen: Res<Screen>| {
                        matches!(*screen, Screen::Message(_) | Screen::CreateWorld(_))
                    }),
                    crate::title_draw::draw.run_if(resource_equals(Screen::Title)),
                    draw_tooltip,
                    crate::title_draw::fade_title.run_if(resource_equals(Screen::Title)),
                    build_gui_batch,
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                (
                    crate::panorama_view::project_panorama_view
                        .run_if(resource_exists::<mcrs_minecraft_render::PanoramaView>),
                    pose_inventory_figure
                        .after(begin_gui_frame)
                        .before(EntitySlotSystems)
                        .run_if(resource_exists::<FigureView>.and_then(
                            resource_equals(Screen::Menu).or_else(resource_changed::<Screen>),
                        )),
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
        use bevy::ecs::system::RunSystemOnce;
        use mcrs_minecraft_item_icon::{IconSources, resolve};
        use mcrs_minecraft_item_stack::{Count, ItemStack, Kind, NameLookup, NameTable};
        use mcrs_minecraft_model::item_model_asset::DisplayContext;
        use mcrs_minecraft_model::item_model_context::ItemModelContext;
        use mcrs_minecraft_model::stack_view::StackView;
        use mcrs_minecraft_resource::ResourceLocation;

        use super::*;
        use crate::menu_test_app::{ATLAS, MODELS, TABLE};

        #[test]
        fn a_stack_overlay_draws_the_count_beside_an_icon() {
            let location = ResourceLocation::read("minecraft:stone").unwrap();
            let kind = Kind(TABLE.id(NameTable::Item, location).unwrap());
            let stack = ItemStack::new(kind, Count::new(17).unwrap());
            let view = StackView {
                stack: &stack,
                table: &TABLE,
            };
            let sources = IconSources {
                models: &MODELS,
                registries: None,
            };
            let icon = resolve(view, &sources, &ItemModelContext::new(DisplayContext::Gui));
            for (overlays, glyph_vertices) in [(Overlays::Stack, 24), (Overlays::None, 0)] {
                let mut scene = GuiScene::default();
                ItemIcons::draw_icon(
                    &mut scene,
                    IVec2::ZERO,
                    icon,
                    StackOverlay::of(view),
                    overlays,
                );
                let mut world = World::new();
                world.insert_resource(scene);
                world.insert_resource(GuiAtlas(Arc::clone(&ATLAS)));
                world.insert_resource(Time::<Real>::default());
                world.insert_resource(GuiConfig {
                    scale: 1,
                    cursor: None,
                    frozen_ticks: Some(0),
                    advanced_tooltips: false,
                });
                world.init_resource::<GlyphCache>();
                world.init_resource::<GuiBatch>();
                world.run_system_once(build_gui_batch).unwrap();
                let batch = world.resource::<GuiBatch>();
                let flat: usize = batch
                    .draws
                    .iter()
                    .filter(|draw| draw.kind == GuiDrawKind::Flat)
                    .map(|draw| draw.range.len())
                    .sum();
                assert_eq!(
                    flat, glyph_vertices,
                    "{overlays:?}: two digits with their shadows, two triangles each"
                );
            }
        }

        #[test]
        fn the_gui_atlas_holds_every_region_and_the_digit_widths() {
            let atlas = GuiAtlasData::load(ResourceSet::corpus()).unwrap();
            for name in GUI_TEXTURES
                .iter()
                .filter(|n| **n != "misc/enchanted_glint_item")
            {
                assert!(atlas.regions.contains_key(name), "{name}");
            }
            for texture in menu_backgrounds() {
                assert!(
                    background_region(texture).is_some_and(|name| atlas.regions.contains_key(name)),
                    "{texture:?}"
                );
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
                    color: WHITE,
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
