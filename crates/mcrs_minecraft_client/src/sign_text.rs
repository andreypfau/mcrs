//! Draws the text of signs and hanging signs (`docs/spec/client-sign-text.md`). The plugin reads
//! the sign entities of the resident columns, builds the glyph quads of each, and hands the
//! whole list to the world text pass. A column is built again when a change message names it
//! and it holds signs, so an embedder that fills the store without the network gets its signs
//! drawn through the same messages the store raises.
//!
//! Not drawn: italic, underline, strikethrough and obfuscation, since a span carries only a
//! colour and bold; code points of the pages other than the ASCII one, which show as the missing
//! glyph; the spyglass case of the outline range.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::math::{DVec3, IVec2, Vec2};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_gui::font::{ASCII_TEXTURE, Font};
use mcrs_minecraft_client_gui::language::ClientLanguage;
use mcrs_minecraft_client_gui::sign_text::{
    self, Glow, MISSING_ADVANCE, OutlineRange, Paint, Side, Sight, SignQuad, packed_light,
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

/// The font page, the font and the language the lines are measured and drawn with.
#[derive(Resource)]
struct SignAssets {
    font: Font,
    language: ClientLanguage,
    page: Handle<Image>,
    /// The size of the page image in texels. It is one row taller than the font page: the extra
    /// row is opaque and gives the solid fill of the missing glyph.
    page_size: Vec2,
}

fn load_assets(
    mut commands: Commands,
    catalog: Res<BlockCatalog>,
    mut images: ResMut<Assets<Image>>,
    mut failed: Local<bool>,
) {
    // A pack that cannot give the font fails the same way on every frame: try once.
    if *failed {
        return;
    }
    let Some(pack) = catalog.pack() else {
        return;
    };
    let loaded = (|| {
        let bytes = pack
            .read(ASCII_TEXTURE)
            .map_err(|error| error.to_string())?;
        let (mut pixels, width, height) =
            mcrs_minecraft_model::atlas::decode_png(bytes, ASCII_TEXTURE)
                .map_err(|error| error.to_string())?;
        let font = Font::load(pack, &pixels, width, height)?.with_missing_advance(MISSING_ADVANCE);
        pixels.extend(std::iter::repeat_n(255, width as usize * 4));
        let mut image = Image::new(
            Extent3d {
                width,
                height: height + 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::nearest();
        let language = ClientLanguage::load(pack)?;
        Ok::<_, String>((
            font,
            language,
            image,
            Vec2::new(width as f32, (height + 1) as f32),
        ))
    })();
    match loaded {
        Ok((font, language, image, page_size)) => commands.insert_resource(SignAssets {
            font,
            language,
            page: images.add(image),
            page_size,
        }),
        Err(reason) => {
            *failed = true;
            error!("the sign text cannot load its font: {reason}");
        }
    }
}

/// The quads of one sign, and whether its outline depends on the distance of the camera.
struct Built {
    vertices: Vec<TextVertex>,
    pos: BlockPos,
    /// A side is glowing and its dye is not black: the outline comes and goes with the range.
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

/// The block and the sky level of a packed light on the scale of the lightmap: a level times 16.
fn lightmap_levels(light: u32) -> [f32; 2] {
    [(light & 0xFF) as f32, ((light >> 20) & 15) as f32 * 16.0]
}

fn push_quad(quad: &SignQuad, cell: [i32; 3], assets: &SignAssets, out: &mut Vec<TextVertex>) {
    let size = assets.font.cell_size;
    let page = assets.page_size;
    let uv = |cell: IVec2, dx: i32, dy: i32| {
        [(cell.x + dx) as f32 / page.x, (cell.y + dy) as f32 / page.y]
    };
    let corners = match quad.cell {
        Some(cell) => [
            uv(cell, 0, 0),
            uv(cell, size.x, 0),
            uv(cell, size.x, size.y),
            uv(cell, 0, size.y),
        ],
        None => [[0.5 / page.x, (page.y - 0.5) / page.y]; 4],
    };
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
    assets: &SignAssets,
) -> Built {
    // A client with no account has the profanity filter off.
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
            push_quad(quad, cell, assets, &mut vertices);
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
    assets: &SignAssets,
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

/// Whether a change at `pos` can alter what is drawn: the column holds signs, or held some.
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
    assets: Res<SignAssets>,
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
        // The messages before the assets loaded are gone, so the first run reads the store.
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
    // chisle: a camera move re-checks every glowing sign every frame and rebuilds the ones that
    // cross the range; an index by distance lifts it when signs number in the thousands.
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
        let built = build_column(&store, &blocks, pos, camera, &assets);
        if built.is_empty() {
            scene.columns.remove(&pos);
        } else {
            scene.columns.insert(pos, built);
        }
    }
    scene.revision += 1;
    // chisle: a change uploads the vertices of every sign again; ranges per column in the
    // buffer lift it when signs number in the tens of thousands.
    text.vertices.clear();
    text.vertices.extend(
        scene
            .columns
            .values()
            .flatten()
            .flat_map(|built| built.vertices.iter().copied()),
    );
    text.texture = Some(assets.page.clone());
    text.revision = scene.revision;
}
