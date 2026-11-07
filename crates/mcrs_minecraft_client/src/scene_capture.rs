use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use crate::Unsettled;
use crate::camera::PlayerCamera;
use crate::stream::Streaming;

const REQUEST: &str = "request";

const REQUEST_POLL: Duration = Duration::from_millis(100);

/// Forty client ticks, the wait of a vanilla capture after its chunks have arrived.
const SETTLE: Duration = Duration::from_millis(40 * 50);

/// Draws the world camera into an image of `size` pixels, with no window and so no HUD. A frame
/// whose name a test writes into `<dir>/request` goes to `<dir>/<name>.png` as 8-bit sRGB with no
/// alpha; the request file is gone before the image appears. The frame is written once the
/// terrain has stayed settled for forty ticks and two shots forty ticks apart are the same, since
/// the light of the terrain keeps changing after its meshes are done.
pub struct SceneCapturePlugin {
    pub dir: PathBuf,
    pub size: UVec2,
}

impl Plugin for SceneCapturePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SceneCapture {
            dir: self.dir.clone(),
            size: self.size,
            target: None,
        })
        .init_resource::<Pending>()
        .add_message::<Unsettled>()
        .add_systems(PostStartup, aim_at_image)
        .add_systems(Update, (read_request, shoot_settled).chain());
    }
}

#[derive(Resource)]
struct SceneCapture {
    dir: PathBuf,
    size: UVec2,
    target: Option<Handle<Image>>,
}

#[derive(Resource, Default)]
struct Pending {
    frame: Option<String>,
    quiet_since: Option<Duration>,
    last_counts: [usize; 3],
    shooting: bool,
    last_shot: Option<Vec<u8>>,
    polled_at: Duration,
}

fn aim_at_image(
    mut capture: ResMut<SceneCapture>,
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

fn read_request(capture: Res<SceneCapture>, time: Res<Time<Real>>, mut pending: ResMut<Pending>) {
    let now = time.elapsed();
    if pending.frame.is_some() || now < pending.polled_at + REQUEST_POLL {
        return;
    }
    pending.polled_at = now;
    let Ok(text) = std::fs::read_to_string(capture.dir.join(REQUEST)) else {
        return;
    };
    let name = text.trim();
    if name.is_empty() || Path::new(name).file_name() != Some(name.as_ref()) {
        return;
    }
    info!(frame = name, "scene frame requested");
    *pending = Pending {
        frame: Some(name.to_owned()),
        polled_at: now,
        ..Pending::default()
    };
}

fn shoot_settled(
    mut commands: Commands,
    capture: Res<SceneCapture>,
    streaming: Streaming,
    time: Res<Time<Real>>,
    mut unsettled: MessageReader<Unsettled>,
    mut pending: ResMut<Pending>,
) {
    let settled = unsettled.read().count() == 0;
    if pending.frame.is_none() || pending.shooting {
        return;
    }
    let status = streaming.status();
    let counts = [status.columns, status.sections, status.sections_total];
    let quiet = settled && streaming.done() && status.columns > 0 && counts == pending.last_counts;
    pending.last_counts = counts;
    let now = time.elapsed();
    if !quiet {
        pending.quiet_since = None;
        return;
    }
    if now < *pending.quiet_since.get_or_insert(now) + SETTLE {
        return;
    }
    let (Some(frame), Some(target)) = (pending.frame.clone(), capture.target.clone()) else {
        return;
    };
    pending.shooting = true;
    let dir = capture.dir.clone();
    commands.spawn(Screenshot::image(target)).observe(
        move |captured: On<ScreenshotCaptured>, mut pending: ResMut<Pending>| {
            if pending.last_shot != captured.image.data {
                pending.last_shot.clone_from(&captured.image.data);
                pending.shooting = false;
                pending.quiet_since = None;
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

fn write_frame(dir: &Path, frame: &str, image: &Image) {
    let image = match image.clone().try_into_dynamic() {
        Ok(image) => image.into_rgb8(),
        Err(error) => {
            error!(frame, %error, "cannot convert the scene frame");
            return;
        }
    };
    let partial = dir.join(format!("{frame}.partial.png"));
    let done = dir.join(format!("{frame}.png"));
    if let Err(error) = image.save(&partial) {
        error!(path = %partial.display(), %error, "cannot write the scene frame");
        return;
    }
    if let Err(error) = std::fs::remove_file(dir.join(REQUEST)) {
        error!(%error, "cannot clear the scene frame request");
    }
    match std::fs::rename(&partial, &done) {
        Ok(()) => info!(path = %done.display(), "scene frame written"),
        Err(error) => error!(path = %done.display(), %error, "cannot move the scene frame"),
    }
}
