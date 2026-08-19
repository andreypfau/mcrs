//! The animation a sprite describes in the `.mcmeta` file beside its `.png`.
//!
//! Mirrors `AnimationMetadataSection` and the sequence `SpriteContents` builds from it: frames are
//! addressed as a grid inside the source image, and the sequence is either the listed one or every
//! frame in order.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    animation: Option<Animation>,
}

#[derive(Deserialize)]
pub struct Animation {
    /// Ticks each step of the sequence lasts, unless the step names its own.
    #[serde(default = "one")]
    frametime: i64,
    width: Option<u32>,
    height: Option<u32>,
    frames: Option<Vec<Frame>>,
}

fn one() -> i64 {
    1
}

/// A step of the sequence: a frame of the image, on its own or with a duration that overrides the
/// default.
#[derive(Deserialize)]
#[serde(untagged)]
enum Frame {
    Index(i64),
    Timed { index: i64, time: Option<i64> },
}

impl Frame {
    fn step(&self, default: i64) -> (i64, i64) {
        match *self {
            Frame::Index(index) => (index, default),
            Frame::Timed { index, time } => (index, time.unwrap_or(default)),
        }
    }
}

/// Reads the metadata beside a sprite. A sprite with no metadata file, or one that says nothing
/// about animation, does not animate.
pub fn read(png: &Path) -> Result<Option<Animation>, String> {
    let mut path = png.as_os_str().to_os_string();
    path.push(".mcmeta");
    let path = PathBuf::from(path);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    serde_json::from_slice::<File>(&bytes)
        .map(|file| file.animation)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

impl Animation {
    /// The size of one frame: both sides where they are given, the image's own side for the one
    /// that is not, and a square of the shorter side where neither is.
    pub fn frame_size(&self, image: (u32, u32)) -> (u32, u32) {
        match (self.width, self.height) {
            (Some(width), Some(height)) => (width, height),
            (Some(width), None) => (width, image.1),
            (None, Some(height)) => (image.0, height),
            (None, None) => {
                let side = image.0.min(image.1);
                (side, side)
            }
        }
    }

    /// How many frames the image holds, laid out as a grid of frame-sized cells.
    pub fn frame_count(&self, image: (u32, u32)) -> u32 {
        let (width, height) = self.frame_size(image);
        if width == 0 || height == 0 {
            return 0;
        }
        image.0 / width * (image.1 / height)
    }

    /// The animation laid out one frame to a step: which frame of the image each step shows, and
    /// how long a step lasts.
    ///
    /// A frame the sequence returns to appears again in the run rather than being pointed at twice,
    /// and a step that lasts longer than the shortest one is repeated until it does. That costs a
    /// duplicate entry but leaves every animation described by a first frame, a length and one
    /// duration, with no schedule to look up.
    pub fn unroll(&self, image: (u32, u32)) -> Vec<u32> {
        let total = self.frame_count(image) as i64;
        let listed: Vec<(i64, i64)> = match &self.frames {
            Some(frames) => frames.iter().map(|frame| frame.step(self.frametime)).collect(),
            None => (0..total).map(|index| (index, self.frametime)).collect(),
        };
        let kept: Vec<(i64, i64)> = listed
            .into_iter()
            .filter(|&(index, time)| time > 0 && (0..total).contains(&index))
            .collect();
        let tick = kept.iter().fold(0, |step, &(_, time)| gcd(step, time));
        if tick == 0 {
            return Vec::new();
        }
        kept.iter()
            .flat_map(|&(index, time)| std::iter::repeat_n(index as u32, (time / tick) as usize))
            .collect()
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}
