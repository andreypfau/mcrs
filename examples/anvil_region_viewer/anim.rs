use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    animation: Option<Animation>,
}

#[derive(Deserialize)]
pub struct Animation {
    #[serde(default = "one")]
    frametime: i64,
    #[serde(default)]
    pub interpolate: bool,
    width: Option<u32>,
    height: Option<u32>,
    frames: Option<Vec<Frame>>,
}

fn one() -> i64 {
    1
}

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

pub fn read(png: &Path) -> Result<Option<Animation>, String> {
    let mut path = png.as_os_str().to_os_string();
    path.push(".mcmeta");
    let path = PathBuf::from(path);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    from_json(&bytes).map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn from_json(bytes: &[u8]) -> Result<Option<Animation>, serde_json::Error> {
    serde_json::from_slice::<File>(bytes).map(|file| file.animation)
}

impl Animation {
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

    pub fn frame_count(&self, image: (u32, u32)) -> u32 {
        let (width, height) = self.frame_size(image);
        if width == 0 || height == 0 {
            return 0;
        }
        image.0 / width * (image.1 / height)
    }

    pub fn unroll(&self, sprite: &str, image: (u32, u32)) -> Unrolled {
        let total = self.frame_count(image) as i64;
        let listed: Vec<(i64, i64)> = match &self.frames {
            Some(frames) => frames.iter().map(|frame| frame.step(self.frametime)).collect(),
            None => (0..total).map(|index| (index, self.frametime)).collect(),
        };
        let mut kept = Vec::with_capacity(listed.len());
        for (step, &(index, time)) in listed.iter().enumerate() {
            if time <= 0 {
                complain(sprite, step, format_args!("lasts {time} ticks"));
            } else if !(0..total).contains(&index) {
                complain(sprite, step, format_args!("names frame {index} of {total}"));
            } else {
                kept.push((index, time));
            }
        }
        let frametime = kept.iter().fold(0, |step, &(_, time)| gcd(step, time)).max(1);
        let frames: Vec<u32> = kept
            .iter()
            .flat_map(|&(index, time)| {
                std::iter::repeat_n(index as u32, (time / frametime) as usize)
            })
            .collect();
        let frames = if frames.len() < 2 { Vec::new() } else { frames };
        Unrolled { frames, frametime: frametime as u32 }
    }
}

pub struct Unrolled {
    pub frames: Vec<u32>,
    pub frametime: u32,
}

fn complain(sprite: &str, step: usize, reason: std::fmt::Arguments) {
    println!("dropping step {step} of {sprite}: it {reason}");
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIDE: u32 = 16;

    fn strip(frames: u32) -> (u32, u32) {
        (SIDE, SIDE * frames)
    }

    fn animation(json: &str) -> Animation {
        from_json(json.as_bytes())
            .expect("the metadata parses")
            .expect("the metadata describes an animation")
    }

    #[test]
    fn an_unlisted_sequence_runs_through_the_whole_image() {
        let meta = animation(r#"{"animation": {"frametime": 2}}"#);
        assert_eq!(meta.unroll("unlisted", strip(4)).frames, [0, 1, 2, 3]);
    }

    #[test]
    fn a_listed_sequence_is_taken_in_the_order_it_is_written() {
        let meta = animation(r#"{"animation": {"frames": [0, 1, 2, 1, 0]}}"#);
        assert_eq!(meta.unroll("listed", strip(3)).frames, [0, 1, 2, 1, 0]);
    }

    #[test]
    fn a_step_with_its_own_duration_is_repeated_to_the_common_beat() {
        let meta = animation(
            r#"{"animation": {"frametime": 2, "frames": [0, {"index": 1, "time": 6}, 2]}}"#,
        );
        let unrolled = meta.unroll("timed", strip(3));
        assert_eq!(unrolled.frames, [0, 1, 1, 1, 2]);
        assert_eq!(unrolled.frametime, 2, "the common beat is the shortest step");
    }

    #[test]
    fn a_step_that_lasts_no_time_is_dropped() {
        let meta = animation(r#"{"animation": {"frames": [0, {"index": 1, "time": -3}, 2]}}"#);
        assert_eq!(meta.unroll("no time", strip(3)).frames, [0, 2]);
    }

    #[test]
    fn a_step_naming_a_frame_the_image_does_not_hold_is_dropped() {
        let meta = animation(r#"{"animation": {"frames": [0, 7, 1]}}"#);
        assert_eq!(meta.unroll("out of range", strip(3)).frames, [0, 1]);
    }

    #[test]
    fn a_sequence_worn_down_to_one_step_does_not_animate() {
        let meta = animation(r#"{"animation": {"frames": [0, 9]}}"#);
        assert!(meta.unroll("worn down", strip(3)).frames.is_empty());
    }

    #[test]
    fn frames_are_counted_across_the_image_as_well_as_down_it() {
        let meta = animation(r#"{"animation": {"width": 16, "height": 16}}"#);
        assert_eq!(meta.frame_count((SIDE * 3, SIDE * 2)), 6);
        assert_eq!(meta.unroll("grid", (SIDE * 3, SIDE * 2)).frames, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn a_frame_is_sized_the_way_the_metadata_leaves_it() {
        assert_eq!(animation(r#"{"animation": {}}"#).frame_size((16, 96)), (16, 16));
        assert_eq!(
            animation(r#"{"animation": {"width": 8}}"#).frame_size((16, 96)),
            (8, 96),
        );
        assert_eq!(
            animation(r#"{"animation": {"height": 4}}"#).frame_size((16, 96)),
            (16, 4),
        );
        assert_eq!(
            animation(r#"{"animation": {"width": 8, "height": 4}}"#).frame_size((16, 96)),
            (8, 4),
        );
    }

    #[test]
    fn metadata_without_an_animation_section_is_not_one() {
        let file = from_json(br#"{"texture": {"blur": true}}"#).expect("the metadata parses");
        assert!(file.is_none());
    }
}
