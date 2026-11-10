use std::fmt::Write;
use std::path::Path;

use image::RgbImage;
use serde::Deserialize;

use crate::run::Paths;

const BACKGROUND: &str = ".background";

/// How far the mask of an entity grows past the pixels that differ.
const MASK_GROWTH: i64 = 2;

/// A person 0.7 m from a monitor 0.7 m wide with 3840 pixels across.
const VIEWING_DISTANCE: f32 = 0.7;
const MONITOR_WIDTH: f32 = 0.7;
const MONITOR_PIXELS: f32 = 3840.0;

#[derive(Clone, Copy, Debug)]
pub struct Thresholds {
    pub in_mask: f32,
    pub out_of_mask: f32,
}

/// The proposed limits of the mean FLIP error of each scene.
pub fn thresholds(scene: &str) -> Thresholds {
    let in_mask = match scene {
        "pig_idle" | "pig_lineup" => 0.05,
        "pig_walk" | "pig_hurt_death" | "frog_states" => 0.08,
        _ => 0.10,
    };
    Thresholds {
        in_mask,
        out_of_mask: 0.10,
    }
}

/// The mean FLIP errors of one mcrs frame against the vanilla frame of the same name.
#[derive(Debug)]
pub struct Score {
    pub scene: String,
    pub frame: String,
    pub mask_pixels: usize,
    /// `None` when the mask is empty, as it is for every background frame.
    pub in_mask: Option<f32>,
    pub out_of_mask: Option<f32>,
}

impl Score {
    pub fn is_background(&self) -> bool {
        self.frame.ends_with(BACKGROUND)
    }

    /// The verdict of the work on entities (CFC-CMP-001, items 5 and 6): the mean in the mask
    /// decides; the mean out of the mask and a background frame are information only.
    pub fn passes(&self, limits: Thresholds) -> bool {
        self.in_mask.is_none_or(|mean| mean <= limits.in_mask)
    }

    /// Whether the mean out of the mask is within its limit, which is information only.
    pub fn out_of_mask_within(&self, limits: Thresholds) -> bool {
        self.out_of_mask
            .is_none_or(|mean| mean <= limits.out_of_mask)
    }
}

#[derive(Deserialize)]
struct Records {
    frames: Vec<Record>,
}

#[derive(Deserialize)]
struct Record {
    image: String,
}

/// Scores each frame of the vanilla record of `scene` and writes its FLIP error image to
/// `<report>/<scene>.<frame>.flip.png`.
pub fn judge(scene: &str, paths: &Paths) -> Vec<Score> {
    let text = paths
        .vanilla_frames
        .file(&format!("{scene}.records.json"))
        .text();
    let records: Records = serde_json::from_str(&text).expect("a record lists its frames");
    std::fs::create_dir_all(paths.report()).expect("the report directory can be created");
    let prefix = format!("{scene}.");
    records
        .frames
        .iter()
        .map(|record| {
            let frame = record
                .image
                .strip_prefix(&prefix)
                .and_then(|name| name.strip_suffix(".png"))
                .unwrap_or_else(|| panic!("{} is not an image of {scene}", record.image));
            score(scene, frame, paths)
        })
        .collect()
}

fn score(scene: &str, frame: &str, paths: &Paths) -> Score {
    let vanilla = vanilla_rgb(paths, &format!("{scene}.{frame}.png"));
    let background_name = if frame.ends_with(BACKGROUND) {
        frame.to_owned()
    } else {
        format!("{frame}{BACKGROUND}")
    };
    let background = vanilla_rgb(paths, &format!("{scene}.{background_name}.png"));
    let mcrs = read_rgb(&paths.frames.join(scene).join(format!("{frame}.png")));
    assert_eq!(
        mcrs.dimensions(),
        vanilla.dimensions(),
        "{scene} {frame}: the mcrs frame and the vanilla frame differ in size"
    );
    let mask = entity_mask(&vanilla, &background);
    let (width, height) = vanilla.dimensions();
    let error = nv_flip::flip(
        nv_flip::FlipImageRgb8::with_data(width, height, vanilla.as_raw()),
        nv_flip::FlipImageRgb8::with_data(width, height, mcrs.as_raw()),
        nv_flip::pixels_per_degree(VIEWING_DISTANCE, MONITOR_PIXELS, MONITOR_WIDTH),
    );
    let map = error.apply_color_lut(&nv_flip::magma_lut());
    let map = RgbImage::from_raw(map.width(), map.height(), map.to_vec())
        .expect("the error image has one RGB pixel for each pixel of the frame");
    let map_path = paths.report().join(format!("{scene}.{frame}.flip.png"));
    map.save(&map_path)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", map_path.display()));

    let mut inside = Mean::default();
    let mut outside = Mean::default();
    for (value, masked) in error.to_vec().into_iter().zip(&mask) {
        if *masked {
            inside.add(value);
        } else {
            outside.add(value);
        }
    }
    Score {
        scene: scene.to_owned(),
        frame: frame.to_owned(),
        mask_pixels: inside.count,
        in_mask: inside.mean(),
        out_of_mask: outside.mean(),
    }
}

#[derive(Default)]
struct Mean {
    sum: f64,
    count: usize,
}

impl Mean {
    fn add(&mut self, value: f32) {
        self.sum += f64::from(value);
        self.count += 1;
    }

    fn mean(&self) -> Option<f32> {
        (self.count > 0).then(|| (self.sum / self.count as f64) as f32)
    }
}

/// The pixels where the frame and its background frame differ in a channel, grown by
/// `MASK_GROWTH` pixels in each direction, in row order.
fn entity_mask(frame: &RgbImage, background: &RgbImage) -> Vec<bool> {
    let (width, height) = frame.dimensions();
    let (width, height) = (i64::from(width), i64::from(height));
    let differs: Vec<bool> = frame
        .pixels()
        .zip(background.pixels())
        .map(|(a, b)| a != b)
        .collect();
    let mut mask = vec![false; differs.len()];
    for y in 0..height {
        for x in 0..width {
            if !differs[(y * width + x) as usize] {
                continue;
            }
            for grown_y in (y - MASK_GROWTH).max(0)..=(y + MASK_GROWTH).min(height - 1) {
                for grown_x in (x - MASK_GROWTH).max(0)..=(x + MASK_GROWTH).min(width - 1) {
                    mask[(grown_y * width + grown_x) as usize] = true;
                }
            }
        }
    }
    mask
}

fn vanilla_rgb(paths: &Paths, name: &str) -> RgbImage {
    image::load_from_memory(&paths.vanilla_frames.file(name).bytes())
        .unwrap_or_else(|error| panic!("cannot decode {name}: {error}"))
        .into_rgb8()
}

fn read_rgb(path: &Path) -> RgbImage {
    image::open(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
        .into_rgb8()
}

/// One line for each score, with the limits and the verdict.
pub fn report(scores: &[Score]) -> String {
    let mut text = String::from("scene\tframe\tmask_px\tin_mask\tout_of_mask\tlimits\tverdict\n");
    for score in scores {
        let limits = thresholds(&score.scene);
        let verdict = match (
            score.is_background(),
            score.passes(limits),
            score.out_of_mask_within(limits),
        ) {
            (true, _, true) => "info",
            (true, _, false) => "info (out of the mask over its limit)",
            (false, true, true) => "pass",
            (false, true, false) => "pass (out of the mask over its limit: information)",
            (false, false, _) => "FAIL",
        };
        let _ = writeln!(
            text,
            "{}\t{}\t{}\t{}\t{}\t{}/{}\t{verdict}",
            score.scene,
            score.frame,
            score.mask_pixels,
            shown(score.in_mask),
            shown(score.out_of_mask),
            limits.in_mask,
            limits.out_of_mask,
        );
    }
    text
}

fn shown(mean: Option<f32>) -> String {
    mean.map_or_else(|| "-".to_owned(), |mean| format!("{mean:.4}"))
}

/// Writes the report of `scores` to `<report>/<name>.txt`, prints it, and fails when the mean in
/// the mask of a frame with entities is over its limit.
pub fn conclude(name: &str, scores: &[Score], paths: &Paths) {
    let text = report(scores);
    let path = paths.report().join(format!("{name}.txt"));
    std::fs::write(&path, &text)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
    println!("{text}report: {}", path.display());
    let failures: Vec<String> = scores
        .iter()
        .filter(|score| !score.is_background() && !score.passes(thresholds(&score.scene)))
        .map(|score| {
            let limits = thresholds(&score.scene);
            format!(
                "{} {}: in the mask {} (limit {}); out of the mask {} (limit {}, information)",
                score.scene,
                score.frame,
                shown(score.in_mask),
                limits.in_mask,
                shown(score.out_of_mask),
                limits.out_of_mask
            )
        })
        .collect();
    assert!(
        failures.is_empty(),
        "frames over their FLIP limits:\n{}",
        failures.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mask_grows_two_pixels_in_each_direction() {
        let background = RgbImage::new(9, 9);
        let mut frame = background.clone();
        frame.put_pixel(4, 4, image::Rgb([0, 0, 1]));
        let mask = entity_mask(&frame, &background);
        for y in 0..9 {
            for x in 0..9 {
                let inside = (2..=6).contains(&x) && (2..=6).contains(&y);
                assert_eq!(mask[y * 9 + x], inside, "pixel ({x}, {y})");
            }
        }
    }
}
