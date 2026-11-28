use std::collections::HashMap;

use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_gui::sign_text::{
    self, Glow, OutlineRange, Paint, Side, Sight, SignQuad, packed_light,
};
use mcrs_minecraft_client_world::columns::{ColumnChange, ColumnStore, Columns, SignEntity};
use mcrs_minecraft_client_world::sign::{SignBlock, sign_block};
use mcrs_minecraft_render::world_text::{TextVertex, WorldText};
use mcrs_minecraft_volume::LightVolume;
use mcrs_voxel::{BlockPos, ColumnPos, LocalPos, SectionPos};

use crate::camera::PlayerCamera;
use crate::glyph_page::{GlyphPage, TextAssets, ensure_loaded, lightmap_levels, linear};

pub struct SignTextPlugin;

impl Plugin for SignTextPlugin {
    fn build(&self, app: &mut App) {
        ensure_loaded(app);
        app.init_resource::<SignScene>().add_systems(
            Update,
            sync_signs
                .run_if(resource_exists::<TextAssets>)
                .run_if(resource_exists::<Blocks>)
                .after(mcrs_minecraft_client_world::columns::ClientTerrainSet::Settle),
        );
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

fn push_quad(quad: &SignQuad, cell: [i32; 3], page: &mut GlyphPage, out: &mut Vec<TextVertex>) {
    let Some(corners) = page.uv_corners(quad) else {
        return;
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
    assets: &mut TextAssets,
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
    assets: &mut TextAssets,
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "a column rebuilds only when its signs change or the camera crosses an outline range, and the built vertices move into `SignScene`"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(5))]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "the revision advances on every rebuild together with the rewritten vertices"
    )
)]
fn sync_signs(
    store: Res<ColumnStore>,
    blocks: Res<Blocks>,
    mut assets: ResMut<TextAssets>,
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
    assets.flush(&mut images);
    scene.revision += 1;
    text.vertices.clear();
    text.vertices.extend(
        scene
            .columns
            .values()
            .flatten()
            .flat_map(|built| built.vertices.iter().copied()),
    );
    if text.texture.as_ref() != Some(&assets.image) {
        text.texture = Some(assets.image.clone());
    }
    text.revision = scene.revision;
}
