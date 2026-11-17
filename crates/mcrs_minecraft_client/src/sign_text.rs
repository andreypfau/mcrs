use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::math::{DVec3, Rect, UVec2, Vec2};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_gui::font::Font;
use mcrs_minecraft_client_gui::font_definition::FontOptions;
use mcrs_minecraft_client_gui::font_glyph::{
    ImageSource, MISSING_GLYPH_SIZE, missing_glyph_pixels,
};
use mcrs_minecraft_client_gui::glyph_cache::{self, CELL, COLUMNS, ROWS, SLOTS};
use mcrs_minecraft_client_gui::language::ClientLanguage;
use mcrs_minecraft_client_gui::sign_text::{
    self, Glow, OutlineRange, Paint, Side, Sight, SignQuad, packed_light,
};
use mcrs_minecraft_client_world::columns::{ColumnChange, ColumnStore, Columns, SignEntity};
use mcrs_minecraft_client_world::sign::{SignBlock, sign_block};
use mcrs_minecraft_render::world_text::{TextVertex, WorldText};
use mcrs_minecraft_volume::LightVolume;
use mcrs_voxel::{BlockPos, ColumnPos, LocalPos, SectionPos};

use crate::camera::PlayerCamera;
use crate::stream::BlockCatalog;

pub struct SignTextPlugin;

impl Plugin for SignTextPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SignScene>().add_systems(
            Update,
            (
                load_assets
                    .run_if(resource_exists::<BlockCatalog>)
                    .run_if(not(resource_exists::<SignAssets>)),
                sync_signs
                    .run_if(resource_exists::<SignAssets>)
                    .run_if(resource_exists::<Blocks>),
            )
                .chain()
                .after(mcrs_minecraft_client_world::columns::ClientTerrainSet::Settle),
        );
    }
}

#[derive(Resource)]
struct SignAssets {
    font: Font,
    language: ClientLanguage,
    page: SignPage,
    image: Handle<Image>,
}

struct SignPage {
    width: u32,
    size: Vec2,
    bitmaps: Vec<(usize, Vec2)>,
    missing: Vec2,
    unifont: Vec2,
    cells: HashMap<char, usize>,
    pending: Vec<(usize, char)>,
}

impl SignPage {
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
        let font = Font::load(pack, FontOptions::default())?;
        let (page, pixels, height) = SignPage::new(&font);
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
        let language = ClientLanguage::load(pack)?;
        Ok::<_, String>(SignAssets {
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
            error!("the sign text cannot load its font: {reason}");
        }
    }
}

struct Built {
    vertices: Vec<TextVertex>,
    pos: BlockPos,
    watches_range: bool,
    in_range: OutlineRange,
}

#[derive(Resource, Default)]
struct SignScene {
    columns: HashMap<ColumnPos, Vec<Built>>,
    revision: u64,
}

fn linear(argb: u32) -> [f32; 3] {
    let color = Color::srgb_u8((argb >> 16) as u8, (argb >> 8) as u8, argb as u8).to_linear();
    [color.red, color.green, color.blue]
}

fn lightmap_levels(light: u32) -> [f32; 2] {
    [(light & 0xFF) as f32, ((light >> 20) & 15) as f32 * 16.0]
}

fn push_quad(quad: &SignQuad, cell: [i32; 3], page: &mut SignPage, out: &mut Vec<TextVertex>) {
    let Some(texels) = page.texels(quad.source, quad.glyph) else {
        return;
    };
    let uv = |x: f32, y: f32| [x / page.size.x, y / page.size.y];
    let (min, max) = (texels.min, texels.max);
    let corners = [
        uv(min.x, min.y),
        uv(max.x, min.y),
        uv(max.x, max.y),
        uv(min.x, max.y),
    ];
    let color = linear(quad.color);
    let light = lightmap_levels(quad.light);
    let vertex = |index: usize| TextVertex {
        block: cell,
        offset: quad.corners[index].to_array(),
        uv: corners[index],
        color,
        light,
    };
    for index in [0, 1, 2, 0, 2, 3] {
        out.push(vertex(index));
    }
}

fn build_sign(
    sign: &SignEntity,
    block: SignBlock,
    light: u8,
    camera: Option<DVec3>,
    assets: &mut SignAssets,
) -> Built {
    const FILTERING: bool = false;
    let in_range = sign_text::outline_range(camera, sign.pos, Sight::Naked);
    let packed = packed_light(light >> 4, light & 15);
    let cell = [sign.pos.x, sign.pos.y, sign.pos.z];
    let mut vertices = Vec::new();
    let mut watches_range = false;
    for (side, text) in [
        (Side::Front, &sign.texts.front_text),
        (Side::Back, &sign.texts.back_text),
    ] {
        watches_range |= text.has_glowing_text && !sign_text::dye_is_black(&text.color);
        let glow = if text.has_glowing_text {
            Glow::Glowing
        } else {
            Glow::Plain
        };
        let paint: Paint = sign_text::paint(&text.color, glow, in_range, packed);
        let lines = sign_text::lines(
            text,
            FILTERING,
            block.family,
            paint.color,
            &assets.font,
            &assets.language,
        );
        let matrix = sign_text::text_matrix(block.family, block.mount, block.angle, side);
        let mut quads = Vec::new();
        sign_text::quads(&assets.font, &lines, &matrix, &paint, &mut quads);
        for quad in &quads {
            push_quad(quad, cell, &mut assets.page, &mut vertices);
        }
    }
    Built {
        vertices,
        pos: sign.pos,
        watches_range,
        in_range,
    }
}

fn build_column(
    store: &ColumnStore,
    blocks: &Blocks,
    at: ColumnPos,
    camera: Option<DVec3>,
    assets: &mut SignAssets,
) -> Vec<Built> {
    let Some(column) = store.get(at) else {
        return Vec::new();
    };
    column
        .signs()
        .iter()
        .filter_map(|sign| {
            let section = store.stored_section(SectionPos::from(sign.pos))?;
            let state = *section.blocks.get(LocalPos::from(sign.pos).index())?;
            let block = sign_block(blocks, state)?;
            let light = store.light(sign.pos).unwrap_or(0);
            Some(build_sign(sign, block, light, camera, assets))
        })
        .collect()
}

fn holds_signs(store: &ColumnStore, scene: &SignScene, pos: ColumnPos) -> bool {
    scene.columns.contains_key(&pos)
        || store
            .get(pos)
            .is_some_and(|column| !column.signs().is_empty())
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and reader is its own parameter"
)]
fn sync_signs(
    store: Res<ColumnStore>,
    blocks: Res<Blocks>,
    mut assets: ResMut<SignAssets>,
    mut images: ResMut<Assets<Image>>,
    camera: Option<Single<&Transform, With<PlayerCamera>>>,
    mut changes: MessageReader<ColumnChange>,
    mut scene: ResMut<SignScene>,
    mut text: ResMut<WorldText>,
    mut stale: Local<Vec<ColumnPos>>,
    mut primed: Local<bool>,
) {
    let camera = camera.map(|transform| transform.translation.as_dvec3());
    let scene = &mut *scene;
    stale.clear();
    if !*primed {
        *primed = true;
        stale.extend(store.resident().map(|(pos, _)| pos));
        changes.clear();
    }
    for change in changes.read() {
        let at = match change {
            ColumnChange::Arrived(pos, _)
            | ColumnChange::Departed(pos, _)
            | ColumnChange::Relit(pos, _) => *pos,
            ColumnChange::Filled(at) => ColumnPos::from(*at),
            ColumnChange::Reblocked(at, _) | ColumnChange::Signed(at) => ColumnPos::from(*at),
        };
        if holds_signs(&store, scene, at) {
            stale.push(at);
        }
    }
    for (pos, signs) in &scene.columns {
        let crossed = signs.iter().any(|built| {
            built.watches_range
                && built.in_range != sign_text::outline_range(camera, built.pos, Sight::Naked)
        });
        if crossed {
            stale.push(*pos);
        }
    }
    stale.sort();
    stale.dedup();
    if stale.is_empty() {
        return;
    }
    for pos in stale.drain(..) {
        let built = build_column(&store, &blocks, pos, camera, &mut assets);
        if built.is_empty() {
            scene.columns.remove(&pos);
        } else {
            scene.columns.insert(pos, built);
        }
    }
    let assets = &mut *assets;
    if !assets.page.pending.is_empty()
        && let Some(mut image) = images.get_mut(&assets.image)
        && let Some(pixels) = image.data.as_mut()
    {
        assets.page.write_pending(&assets.font, pixels);
    }
    scene.revision += 1;
    text.vertices.clear();
    text.vertices.extend(
        scene
            .columns
            .values()
            .flatten()
            .flat_map(|built| built.vertices.iter().copied()),
    );
    text.texture = Some(assets.image.clone());
    text.revision = scene.revision;
}
