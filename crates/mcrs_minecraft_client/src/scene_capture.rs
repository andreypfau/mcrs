use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use crate::Unsettled;
use crate::camera::PlayerCamera;
use crate::config::Knobs;
use crate::scene_record::{PlaceFrame, SceneRecord, place_recorded_entities};
use crate::settle::Settle;
use crate::stream::Streaming;

const REQUEST: &str = "request";

const REQUEST_POLL: Duration = Duration::from_millis(100);

/// Forty client ticks, the wait of a vanilla capture after its chunks have arrived.
const SETTLE: Duration = Duration::from_millis(40 * 50);

/// How long the shots of a frame can differ before the frame fails (CFC-RUN-004).
const SETTLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Draws the world camera into an image of `size` pixels, with no window and so no HUD. A frame
/// whose name a test writes into `<dir>/request` goes to `<dir>/<name>.png` as 8-bit sRGB with no
/// alpha; the request file is gone before the image appears, and also when the frame fails. The
/// frame is written once the terrain has stayed settled for forty ticks and two shots forty ticks
/// apart are the same, since the light of the terrain keeps changing after its meshes are done.
pub struct SceneCapturePlugin {
    pub dir: PathBuf,
    pub size: UVec2,
    /// The record of the vanilla client that gives the entities of each frame, in the render
    /// mode of a comparison; `None` takes them from the server.
    pub record: Option<PathBuf>,
}

impl Knobs {
    /// `SCENE_DIR=<dir>` runs with no window: the world camera draws into an image of
    /// `RESOLUTION`, and each frame that `<dir>/request` names is written to `<dir>/<name>.png`.
    pub fn scene_dir(&self) -> Option<PathBuf> {
        self.knob("SCENE_DIR").map(PathBuf::from)
    }

    /// `SCENE_RECORD=<record>` takes the entities of each scene frame from the frame of the same
    /// name in the record of the vanilla client, the render mode of a comparison.
    pub fn scene_record(&self) -> Option<PathBuf> {
        self.knob("SCENE_RECORD").map(PathBuf::from)
    }
}

impl Plugin for SceneCapturePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FrameTarget {
            dir: self.dir.clone(),
            size: self.size,
            target: None,
        })
        .init_resource::<Pending>()
        .add_message::<PlaceFrame>()
        .add_message::<Unsettled>()
        .add_systems(PostStartup, aim_at_image)
        .add_systems(Update, (read_request, shoot_settled).chain());
        if let Some(path) = &self.record {
            match SceneRecord::read(path) {
                Ok(record) => {
                    app.insert_resource(record).add_systems(
                        Update,
                        place_recorded_entities
                            .after(read_request)
                            .before(shoot_settled),
                    );
                }
                Err(error) => {
                    error!(path = %path.display(), %error, "cannot read the scene record");
                }
            }
        }
    }
}

#[derive(Resource)]
struct FrameTarget {
    dir: PathBuf,
    size: UVec2,
    target: Option<Handle<Image>>,
}

#[derive(Resource, Default)]
struct Pending {
    frame: Option<String>,
    settle: Settle,
    shooting: bool,
    last_shot: Option<Vec<u8>>,
    polled_at: Duration,
    /// When the first shot of the frame was taken.
    first_shot: Option<Duration>,
}

fn aim_at_image(
    mut capture: ResMut<FrameTarget>,
    mut camera: Single<&mut RenderTarget, With<PlayerCamera>>,
    mut images: ResMut<Assets<Image>>,
) {
    let handle = images.add(Image::new_target_texture(
        capture.size.x,
        capture.size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    **camera = RenderTarget::Image(ImageRenderTarget {
        handle: handle.clone(),
        scale_factor: 1.0,
    });
    capture.target = Some(handle);
    if let Err(error) = std::fs::create_dir_all(&capture.dir) {
        error!(dir = %capture.dir.display(), %error, "cannot create the scene frame directory");
    }
    info!(dir = %capture.dir.display(), size = ?capture.size, "drawing scene frames off screen");
}

fn read_request(
    capture: Res<FrameTarget>,
    time: Res<Time<Real>>,
    mut places: MessageWriter<PlaceFrame>,
    mut pending: ResMut<Pending>,
) {
    let now = time.elapsed();
    if pending.frame.is_some() || now < pending.polled_at + REQUEST_POLL {
        return;
    }
    pending.polled_at = now;
    let Ok(text) = std::fs::read_to_string(capture.dir.join(REQUEST)) else {
        return;
    };
    let name = text.trim();
    if !is_frame_name(name) {
        return;
    }
    info!(frame = name, "scene frame requested");
    places.write(PlaceFrame(name.to_owned()));
    *pending = Pending {
        frame: Some(name.to_owned()),
        polled_at: now,
        ..Pending::default()
    };
}

/// A plain file name that does not collide with the files the capture writes next to the frame.
fn is_frame_name(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name).file_name() == Some(name.as_ref())
        && !name.ends_with(".partial")
        && !name.ends_with(".unsettled")
}

fn shoot_settled(
    mut commands: Commands,
    capture: Res<FrameTarget>,
    streaming: Streaming,
    time: Res<Time<Real>>,
    mut unsettled: MessageReader<Unsettled>,
    mut pending: ResMut<Pending>,
) {
    if pending.frame.is_none() || pending.shooting {
        unsettled.clear();
        return;
    }
    let now = time.elapsed();
    let quiet = pending.settle.observe(now, &streaming, &mut unsettled);
    if quiet.is_none_or(|quiet| quiet < SETTLE) {
        return;
    }
    let (Some(frame), Some(target)) = (pending.frame.clone(), capture.target.clone()) else {
        return;
    };
    pending.shooting = true;
    pending.first_shot.get_or_insert(now);
    let dir = capture.dir.clone();
    commands.spawn(Screenshot::image(target)).observe(
        move |captured: On<ScreenshotCaptured>,
              time: Res<Time<Real>>,
              mut pending: ResMut<Pending>| {
            if pending.last_shot != captured.image.data {
                let started = pending.first_shot.unwrap_or_default();
                if time.elapsed().saturating_sub(started) > SETTLE_TIMEOUT {
                    error!(frame, "the frame did not settle in {SETTLE_TIMEOUT:?}");
                    let _ = std::fs::write(dir.join(format!("{frame}.unsettled")), "");
                    remove_request(&dir);
                    *pending = Pending {
                        polled_at: pending.polled_at,
                        ..Pending::default()
                    };
                    return;
                }
                pending.last_shot.clone_from(&captured.image.data);
                pending.shooting = false;
                pending.settle.restart();
                return;
            }
            write_frame(&dir, &frame, &captured.image);
            *pending = Pending {
                polled_at: pending.polled_at,
                ..Pending::default()
            };
        },
    );
}

fn remove_request(dir: &Path) {
    if let Err(error) = std::fs::remove_file(dir.join(REQUEST)) {
        error!(%error, "cannot clear the scene frame request");
    }
}

fn write_frame(dir: &Path, frame: &str, image: &Image) {
    let image = match image.clone().try_into_dynamic() {
        Ok(image) => image.into_rgb8(),
        Err(error) => {
            error!(frame, %error, "cannot convert the scene frame");
            remove_request(dir);
            return;
        }
    };
    let partial = dir.join(format!("{frame}.partial.png"));
    let done = dir.join(format!("{frame}.png"));
    if let Err(error) = image.save(&partial) {
        error!(path = %partial.display(), %error, "cannot write the scene frame");
        remove_request(dir);
        return;
    }
    remove_request(dir);
    match std::fs::rename(&partial, &done) {
        Ok(()) => info!(path = %done.display(), "scene frame written"),
        Err(error) => error!(path = %done.display(), %error, "cannot move the scene frame"),
    }
}
