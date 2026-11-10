//! Draws the text of signs and hanging signs (`docs/spec/client-sign-text.md`). The plugin reads
//! the sign entities of the resident columns, builds the glyph quads of each, and hands the
//! whole list to the world text pass.
//!
//! A column the plugin has not seen, or one whose handle moved, has its signs built again. Every
//! change of a column goes through `Arc::make_mut`, and the plugin keeps a `Weak` to each handle,
//! so a changed column always has a new handle. The plugin reads the store and no message, so an
//! embedder that fills the store without the network (`ColumnStore::insert`, then
//! `ColumnStore::set_sign` or `Column::with_signs`) gets its signs drawn too.
//!
//! Not drawn: italic, underline, strikethrough and obfuscation, since a span carries only a
//! colour and bold; code points of the pages other than the ASCII one, which show as the missing
//! glyph; the spyglass case of the outline range.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::math::{DVec3, IVec2, Vec2, Vec3};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_gui::font::{ASCII_TEXTURE, Font};
use mcrs_minecraft_client_gui::language::ClientLanguage;
use mcrs_minecraft_client_gui::sign_text::{
    self, MISSING_ADVANCE, Paint, Side, SignQuad, packed_light,
};
use mcrs_minecraft_client_world::columns::{Column, ColumnStore, Columns, SignEntity};
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
) {
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
        Err(reason) => error!("the sign text cannot load its font: {reason}"),
    }
}

/// The quads of one sign, and whether its outline depends on the distance of the camera.
struct Built {
    vertices: Vec<TextVertex>,
    pos: BlockPos,
    /// A side is glowing and its dye is not black: the outline comes and goes with the range.
    watches_range: bool,
    in_range: bool,
}

#[derive(Resource, Default)]
struct SignScene {
    columns: HashMap<ColumnPos, Vec<Built>>,
    seen: HashMap<ColumnPos, Weak<Column>>,
    revision: u64,
}

/// SIGN-LIGHT-001 gives the light as numbers and the frame takes a colour, so the light of a
/// line is the brightness of the brighter of its two levels on the game's own curve.
fn brightness(light: u32) -> f32 {
    let level = ((light >> 4) & 15).max((light >> 20) & 15) as f32 / 15.0;
    0.05 + 0.95 * level / (4.0 - 3.0 * level)
}

fn linear(argb: u32, brightness: f32) -> [f32; 4] {
    let color = Color::srgb_u8((argb >> 16) as u8, (argb >> 8) as u8, argb as u8).to_linear();
    [
        color.red * brightness,
        color.green * brightness,
        color.blue * brightness,
        1.0,
    ]
}

fn push_quad(quad: &SignQuad, origin: Vec3, assets: &SignAssets, out: &mut Vec<TextVertex>) {
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
    let color = linear(quad.color, brightness(quad.light));
    let vertex = |index: usize| TextVertex {
        position: (quad.corners[index] + origin).to_array(),
        uv: corners[index],
        color,
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
    let in_range = sign_text::outline_in_range(camera, sign.pos, false);
    let packed = packed_light(light >> 4, light & 15);
    let origin = Vec3::new(sign.pos.x as f32, sign.pos.y as f32, sign.pos.z as f32);
    let mut vertices = Vec::new();
    let mut watches_range = false;
    for (side, text) in [
        (Side::Front, &sign.texts.front_text),
        (Side::Back, &sign.texts.back_text),
    ] {
        watches_range |=
            text.has_glowing_text && sign_text::dye_colors(&text.color).0 != 0xFF00_0000;
        let paint: Paint = sign_text::paint(&text.color, text.has_glowing_text, in_range, packed);
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
            push_quad(quad, origin, assets, &mut vertices);
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

fn sync_signs(
    store: Res<ColumnStore>,
    blocks: Res<Blocks>,
    assets: Res<SignAssets>,
    camera: Option<Single<&Transform, With<PlayerCamera>>>,
    mut scene: ResMut<SignScene>,
    mut text: ResMut<WorldText>,
) {
    let camera = camera.map(|transform| transform.translation.as_dvec3());
    let scene = &mut *scene;
    let mut stale: Vec<ColumnPos> = Vec::new();
    for (pos, handle) in store.resident() {
        let current = Arc::as_ptr(handle);
        if scene
            .seen
            .get(&pos)
            .is_none_or(|seen| seen.as_ptr() != current)
        {
            scene.seen.insert(pos, Arc::downgrade(handle));
            stale.push(pos);
        }
    }
    scene.seen.retain(|pos, _| {
        let held = store.holds(*pos);
        if !held {
            stale.push(*pos);
        }
        held
    });
    // ponytail: a camera move re-checks every glowing sign every frame and rebuilds the
    // ones that cross the range; an index by distance if signs ever number in the thousands.
    for (pos, signs) in &scene.columns {
        let crossed = signs.iter().any(|built| {
            built.watches_range
                && built.in_range != sign_text::outline_in_range(camera, built.pos, false)
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
    for pos in stale {
        let built = build_column(&store, &blocks, pos, camera, &assets);
        if built.is_empty() {
            scene.columns.remove(&pos);
        } else {
            scene.columns.insert(pos, built);
        }
    }
    scene.revision += 1;
    text.vertices = Arc::new(
        scene
            .columns
            .values()
            .flatten()
            .flat_map(|built| built.vertices.iter().copied())
            .collect(),
    );
    text.texture = Some(assets.page.clone());
    text.revision = scene.revision;
}
