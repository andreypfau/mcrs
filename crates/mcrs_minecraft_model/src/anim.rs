use serde::Deserialize;

use crate::model::Pack;

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

pub fn read(pack: &Pack, png: &str) -> Result<Option<Animation>, String> {
    let path = format!("{png}.mcmeta");
    let Some(bytes) = pack.get(&path) else {
        return Ok(None);
    };
    from_json(bytes).map_err(|error| format!("cannot parse {path}: {error}"))
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
            Some(frames) => frames
                .iter()
                .map(|frame| frame.step(self.frametime))
                .collect(),
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
        let frametime = kept
            .iter()
            .fold(0, |step, &(_, time)| gcd(step, time))
            .max(1);
        let frames: Vec<u32> = kept
            .iter()
            .flat_map(|&(index, time)| {
                std::iter::repeat_n(index as u32, (time / frametime) as usize)
            })
            .collect();
        let frames = if frames.len() < 2 { Vec::new() } else { frames };
        Unrolled {
            frames,
            frametime: frametime as u32,
        }
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
    fn metadata_unrolls_into_frames_on_the_common_beat() {
        let timed = animation(
            r#"{"animation": {"frametime": 2, "frames": [0, {"index": 1, "time": 6}, 2]}}"#,
        )
        .unroll("timed", strip(3));
        assert_eq!(timed.frames, [0, 1, 1, 1, 2]);
        assert_eq!(timed.frametime, 2, "the common beat is the shortest step");

        let worn_down = animation(r#"{"animation": {"frames": [0, 9]}}"#);
        assert!(worn_down.unroll("worn down", strip(3)).frames.is_empty());

        let grid = animation(r#"{"animation": {"width": 16, "height": 16}}"#);
        assert_eq!(grid.frame_count((SIDE * 3, SIDE * 2)), 6);
        assert_eq!(
            grid.unroll("grid", (SIDE * 3, SIDE * 2)).frames,
            [0, 1, 2, 3, 4, 5]
        );

        for (json, size) in [
            (r#"{"animation": {}}"#, (16, 16)),
            (r#"{"animation": {"width": 8}}"#, (8, 96)),
            (r#"{"animation": {"height": 4}}"#, (16, 4)),
            (r#"{"animation": {"width": 8, "height": 4}}"#, (8, 4)),
        ] {
            assert_eq!(animation(json).frame_size((16, 96)), size, "{json}");
        }

        let still = from_json(br#"{"texture": {"blur": true}}"#).expect("the metadata parses");
        assert!(still.is_none());
    }
}
