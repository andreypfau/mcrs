use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mcrs_minecraft_client_jar::{self as client_jar, Files, Progress};

use crate::atlas::decode_png;
use crate::gui::font::{ASCII_TEXTURE, Font, Span, draw_centered_text};
use crate::gui::scene::GuiQuad;
use crate::model::{Pack, is_resource};

/// The asset source holding the resource pack of the vanilla client jar.
pub const SOURCE: &str = "vanilla";

const BAR_WIDTH: f32 = 400.0;
const TEXT_SCALE: f32 = 2.0;
const WHITE: u32 = 0xFFFF_FFFF;
const GRAY: u32 = 0xFFA0_A0A0;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VanillaAssets {
    #[default]
    Fetching,
    Ready,
}

#[derive(Resource)]
struct Fetch {
    progress: Arc<Progress>,
    fonts: Arc<Mutex<Option<Files>>>,
    unpacked: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

/// Registers the `vanilla` source and starts filling it. Must run before `AssetPlugin` builds.
pub fn register(app: &mut App) {
    let root = Dir::default();
    register_source(app, root.clone());
    let progress = Arc::<Progress>::default();
    let fonts = Arc::<Mutex<Option<Files>>>::default();
    let unpacked = Arc::<AtomicBool>::default();
    #[cfg(not(target_family = "wasm"))]
    let worker = Some(spawn(
        root,
        progress.clone(),
        fonts.clone(),
        unpacked.clone(),
    ));
    #[cfg(target_family = "wasm")]
    let worker = {
        spawn_web(root, progress.clone(), fonts.clone(), unpacked.clone());
        None
    };
    app.insert_resource(Fetch {
        progress,
        fonts,
        unpacked,
        worker,
    });
}

// A dedicated thread rather than the IO pool: a download can block for minutes, and the pool
// is shared with `Pack::load` and every other asset load.
#[cfg(not(target_family = "wasm"))]
fn spawn(
    root: Dir,
    progress: Arc<Progress>,
    fonts: Arc<Mutex<Option<Files>>>,
    unpacked: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("client jar".to_owned())
        .spawn(move || {
            let (files, rest) = client_jar::resolve(&progress, is_resource, |files| {
                *fonts.lock().unwrap() = Some(files);
            });
            fill(&root, files);
            unpacked.store(true, Ordering::Release);
            if let Some(rest) = rest {
                rest.finish();
            }
        })
        .expect("a thread for the client jar")
}

#[cfg(target_family = "wasm")]
fn spawn_web(
    root: Dir,
    progress: Arc<Progress>,
    fonts: Arc<Mutex<Option<Files>>>,
    unpacked: Arc<AtomicBool>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let files = client_jar::fetch(&progress, is_resource, |files| {
            *fonts.lock().unwrap() = Some(files);
        })
        .await;
        fill(&root, files);
        unpacked.store(true, Ordering::Release);
    });
}

pub fn register_source(app: &mut App, root: Dir) {
    app.register_asset_source(
        AssetSourceId::from(SOURCE),
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: root.clone() })),
    );
}

// ponytail: the resource half is held twice, here and again in `Pack`; have `Pack` hold the
// `Dir`'s `Arc<Vec<u8>>` values instead if the memory matters.
pub fn fill(root: &Dir, files: Files) {
    for (path, bytes) in files {
        root.insert_asset(Path::new(&path), bytes);
    }
}

#[cfg(test)]
pub fn resource_files() -> &'static Files {
    static FILES: std::sync::LazyLock<Files> =
        std::sync::LazyLock::new(|| client_jar::resolve(&Progress::default(), is_resource, drop).0);
    &FILES
}

pub struct VanillaAssetsPlugin;

impl Plugin for VanillaAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<VanillaAssets>()
            .add_systems(OnEnter(VanillaAssets::Fetching), spawn_overlay)
            .add_systems(
                Update,
                (
                    finish_fetch,
                    load_font.run_if(not(resource_exists::<LoadingFont>)),
                    draw_progress,
                )
                    .chain()
                    .run_if(in_state(VanillaAssets::Fetching)),
            );
    }
}

fn finish_fetch(mut fetch: ResMut<Fetch>, mut next: ResMut<NextState<VanillaAssets>>) {
    if fetch.unpacked.load(Ordering::Acquire) {
        next.set(VanillaAssets::Ready);
        return;
    }
    if fetch.worker.as_ref().is_some_and(JoinHandle::is_finished)
        && let Err(panic) = fetch.worker.take().unwrap().join()
    {
        std::panic::resume_unwind(panic);
    }
}

/// The game's own font, for the loading screen, before the rest of the pack is in.
#[derive(Resource)]
struct LoadingFont {
    font: Font,
    page: Handle<Image>,
}

fn load_font(fetch: Res<Fetch>, mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let Some(files) = fetch.fonts.lock().unwrap().take() else {
        return;
    };
    match font_page(files) {
        Ok((font, page)) => commands.insert_resource(LoadingFont {
            font,
            page: images.add(page),
        }),
        Err(error) => warn!("the loading screen stays without text: {error}"),
    }
}

fn font_page(files: Files) -> Result<(Font, Image), String> {
    let pack: Pack = files.into_iter().collect();
    let (pixels, width, height) = decode_png(pack.read(ASCII_TEXTURE)?, ASCII_TEXTURE)?;
    let font = Font::load(&pack, &pixels, width, height)?;
    let mut page = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    page.sampler = ImageSampler::nearest();
    Ok((font, page))
}

#[derive(Component)]
struct ProgressFill;

#[derive(Component, Clone, Copy)]
enum Line {
    Amount,
    Status,
}

fn spawn_overlay(mut commands: Commands) {
    let line = |line: Line| {
        (
            line,
            Node {
                width: Val::Px(BAR_WIDTH),
                height: Val::Px(10.0 * TEXT_SCALE),
                ..default()
            },
        )
    };
    commands.spawn((
        DespawnOnExit(VanillaAssets::Fetching),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.08, 0.08, 0.1)),
        GlobalZIndex(i32::MAX),
        children![
            line(Line::Amount),
            (
                Node {
                    width: Val::Px(BAR_WIDTH),
                    height: Val::Px(4.0 * TEXT_SCALE),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.25, 0.25, 0.28)),
                children![(
                    ProgressFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                )],
            ),
            line(Line::Status),
        ],
    ));
}

fn draw_progress(
    fetch: Res<Fetch>,
    font: Option<Res<LoadingFont>>,
    fill: Single<&mut Node, With<ProgressFill>>,
    lines: Query<(Entity, &Line)>,
    mut shown: Local<[String; 2]>,
    mut commands: Commands,
) {
    let (done, total) = (fetch.progress.done(), fetch.progress.total());
    let fraction = if total == 0 {
        0.0
    } else {
        (done as f64 / total as f64).min(1.0)
    };
    fill.into_inner()
        .map_unchanged(|node| &mut node.width)
        .set_if_neq(Val::Percent(100.0 * fraction as f32));
    let Some(font) = font else {
        return;
    };
    for (entity, &line) in &lines {
        let (text, color) = match line {
            Line::Amount => (
                format!(
                    "{:.1} / {:.1} MB  {:.0}%",
                    done as f64 / 1e6,
                    total as f64 / 1e6,
                    100.0 * fraction
                ),
                WHITE,
            ),
            Line::Status => (fetch.progress.status(), GRAY),
        };
        if shown[line as usize] == text {
            continue;
        }
        let mut glyphs = Vec::new();
        draw_centered_text(
            &font.font,
            (BAR_WIDTH / TEXT_SCALE) as i32 / 2,
            0,
            &[Span::new(text.as_str(), color)],
            &mut glyphs,
        );
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|row| {
                for glyph in glyphs {
                    if let Some(node) = glyph_node(&font, glyph) {
                        row.spawn(node);
                    }
                }
            });
        shown[line as usize] = text;
    }
}

fn glyph_node(font: &LoadingFont, quad: GuiQuad) -> Option<(Node, ImageNode)> {
    let GuiQuad::Glyph {
        origin,
        glyph,
        color,
    } = quad
    else {
        return None;
    };
    let cell = font.font.glyph(glyph)?.cell.as_vec2();
    let size = font.font.cell_size.as_vec2();
    let [a, r, g, b] = color.to_be_bytes();
    Some((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(origin.x as f32 * TEXT_SCALE),
            top: Val::Px(origin.y as f32 * TEXT_SCALE),
            width: Val::Px(size.x * TEXT_SCALE),
            height: Val::Px(size.y * TEXT_SCALE),
            ..default()
        },
        ImageNode {
            image: font.page.clone(),
            rect: Some(Rect::from_corners(cell, cell + size)),
            color: Color::srgba_u8(r, g, b, a),
            ..default()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delivered_font_files_draw_the_progress_line() {
        let mut delivered = None;
        client_jar::resolve(
            &Progress::default(),
            |_| false,
            |files| delivered = Some(files),
        );

        let (font, page) = font_page(delivered.expect("fonts are delivered")).unwrap();

        assert_eq!(page.width(), 128);
        for ch in "0123456789./ MB%".chars() {
            assert!(font.advance(ch, false) > 0, "{ch:?} has no advance");
        }
    }
}
