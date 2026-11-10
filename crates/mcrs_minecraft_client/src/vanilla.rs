use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::memory::Dir;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mcrs_minecraft_client_jar::{self as client_jar, Files, Progress};

use mcrs_minecraft_client_gui::font::{Font, Span, draw_centered_text};
use mcrs_minecraft_client_gui::font_definition::FontOptions;
use mcrs_minecraft_client_gui::font_glyph::ImageSource;
use mcrs_minecraft_client_gui::scene::GuiQuad;
use mcrs_minecraft_model::resource_set::{ResourceSet, is_resource};
use mcrs_minecraft_model::vanilla_source::{fill, register_source};

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
// is shared with `ResourceSet::load` and every other asset load.
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
            // The font objects come from another host, so they are fetched while the jar is.
            let (sender, font_layer) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = sender.send(client_jar::font_assets());
            });
            let resolved = client_jar::resolve(&progress, is_resource, |files| {
                *fonts.lock().unwrap() = Some(files);
            });
            let (files, rest) = match resolved {
                Ok(resolved) => resolved,
                Err(error) => {
                    *progress.status.lock().unwrap() = error.to_string();
                    return;
                }
            };
            fill(&root, files);
            // The asset index layer lies above the jar, so its files are laid second.
            match await_layer(&font_layer, FONT_LAYER_DEADLINE) {
                Ok(layer) => fill(&root, layer),
                Err(error) => warn!("the text keeps to the jar's glyphs: {error}"),
            }
            unpacked.store(true, Ordering::Release);
            if let Some(rest) = rest {
                rest.finish();
            }
        })
        .expect("a thread for the client jar")
}

/// One overall wait for the font layer, so a stalling link cannot hold startup through every
/// retry; the fetch thread is left to finish on its own.
#[cfg(not(target_family = "wasm"))]
const FONT_LAYER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

#[cfg(not(target_family = "wasm"))]
fn await_layer(
    layer: &std::sync::mpsc::Receiver<Result<Files, String>>,
    deadline: std::time::Duration,
) -> Result<Files, String> {
    use std::sync::mpsc::RecvTimeoutError;
    match layer.recv_timeout(deadline) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(format!("the font fetch took over {deadline:?}")),
        Err(RecvTimeoutError::Disconnected) => Err("the font fetch panicked".to_owned()),
    }
}

#[cfg(target_family = "wasm")]
fn spawn_web(
    root: Dir,
    progress: Arc<Progress>,
    fonts: Arc<Mutex<Option<Files>>>,
    unpacked: Arc<AtomicBool>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        // The font objects come from another host, so they are fetched while the jar is.
        let (files, layer) = futures_lite::future::zip(
            client_jar::fetch(&progress, is_resource, |files| {
                *fonts.lock().unwrap() = Some(files);
            }),
            client_jar::font_assets(),
        )
        .await;
        fill(&root, files);
        match layer {
            Ok(layer) => fill(&root, layer),
            Err(error) => warn!("the text keeps to the jar's glyphs: {error}"),
        }
        unpacked.store(true, Ordering::Release);
    });
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
    /// The image of each bitmap provider's texture, by the provider's index.
    pages: Vec<(usize, Handle<Image>)>,
}

fn load_font(fetch: Res<Fetch>, mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let Some(files) = fetch.fonts.lock().unwrap().take() else {
        return;
    };
    match font_pages(files) {
        Ok((font, pages)) => commands.insert_resource(LoadingFont {
            font,
            pages: pages
                .into_iter()
                .map(|(source, page)| (source, images.add(page)))
                .collect(),
        }),
        Err(error) => warn!("the loading screen stays without text: {error}"),
    }
}

fn font_pages(files: Files) -> Result<(Font, Vec<(usize, Image)>), String> {
    let pack: ResourceSet = files.into_iter().collect();
    let font = Font::load(&pack, FontOptions::default())?;
    let pages = font
        .bitmap_pages()
        .map(|(source, page)| {
            let (width, height) = page.size();
            let mut image = Image::new(
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                page.pixels().to_vec(),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            image.sampler = ImageSampler::nearest();
            (source, image)
        })
        .collect();
    Ok((font, pages))
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

#[derive(Default)]
struct Rate {
    at: f64,
    done: u64,
    bytes_per_second: f64,
}

impl Rate {
    const INTERVAL: f64 = 0.5;

    fn sample(&mut self, now: f64, done: u64) {
        if self.at == 0.0 {
            (self.at, self.done) = (now, done);
            return;
        }
        let elapsed = now - self.at;
        if elapsed < Self::INTERVAL {
            return;
        }
        let rate = done.saturating_sub(self.done) as f64 / elapsed;
        self.bytes_per_second = if self.bytes_per_second == 0.0 {
            rate
        } else {
            0.7 * self.bytes_per_second + 0.3 * rate
        };
        (self.at, self.done) = (now, done);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
fn draw_progress(
    fetch: Res<Fetch>,
    font: Option<Res<LoadingFont>>,
    time: Res<Time<Real>>,
    fill: Single<&mut Node, With<ProgressFill>>,
    lines: Query<(Entity, &Line)>,
    mut rate: Local<Rate>,
    mut shown: Local<[String; 2]>,
    mut commands: Commands,
) {
    let (done, total) = (
        fetch.progress.done.load(Ordering::Relaxed),
        fetch.progress.total.load(Ordering::Relaxed),
    );
    rate.sample(time.elapsed_secs_f64(), done);
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
            Line::Amount => {
                let mut text = format!(
                    "{:.1} / {:.1} MB  {:.0}%",
                    done as f64 / 1e6,
                    total as f64 / 1e6,
                    100.0 * fraction
                );
                if rate.bytes_per_second > 0.0 {
                    text += &format!(
                        "  {:.1} MB/s  {:.0} s left",
                        rate.bytes_per_second / 1e6,
                        total.saturating_sub(done) as f64 / rate.bytes_per_second
                    );
                }
                (text, WHITE)
            }
            Line::Status => (fetch.progress.status.lock().unwrap().clone(), GRAY),
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

/// The node of a glyph the loading screen can draw: one from a bitmap page, which is all the jar
/// alone gives.
fn glyph_node(font: &LoadingFont, quad: GuiQuad) -> Option<(Node, ImageNode)> {
    let GuiQuad::Glyph {
        pen,
        image,
        color,
        weight,
        ..
    } = quad
    else {
        return None;
    };
    let ImageSource::Bitmap { source, texels } = image.source else {
        return None;
    };
    let (_, page) = font.pages.iter().find(|(page, _)| *page == source)?;
    let rect = image.strike(pen, weight);
    let [a, r, g, b] = color.to_be_bytes();
    Some((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.min.x * TEXT_SCALE),
            top: Val::Px(rect.min.y * TEXT_SCALE),
            width: Val::Px(rect.width() * TEXT_SCALE),
            height: Val::Px(rect.height() * TEXT_SCALE),
            ..default()
        },
        ImageNode {
            image: page.clone(),
            rect: Some(texels),
            color: Color::srgba_u8(r, g, b, a),
            ..default()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_family = "wasm"))]
    mod layer_deadline {
        use std::time::Duration;

        use super::*;

        #[test]
        fn a_silent_fetch_times_out() {
            let (_sender, receiver) = std::sync::mpsc::channel();
            let error = await_layer(&receiver, Duration::from_millis(20)).unwrap_err();
            assert!(error.contains("took over"), "{error}");
        }
    }

    mod jar {
        use super::*;

        #[test]
        fn the_delivered_font_files_draw_the_progress_line() {
            let mut delivered = None;
            client_jar::resolve(
                &Progress::default(),
                |_| false,
                |files| delivered = Some(files),
            )
            .unwrap();

            let (font, pages) = font_pages(delivered.expect("fonts are delivered")).unwrap();

            assert_eq!(pages.len(), 3);
            assert!(pages.iter().any(|(_, page)| page.width() == 128));
            for ch in "0123456789./ MB%".chars() {
                assert!(font.advance(ch, false) > 0.0, "{ch:?} has no advance");
            }
        }
    }
}
