//! Screenshot parity for the hotbar and the inventory screen against fixtures captured
//! from the vanilla client (`tests/fixtures/gui/README.md`). The comparison helpers run
//! everywhere; the capture itself needs a window and runs only under `MCRS_GUI_PARITY=1`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};

const WIDTH: u32 = 854;
const HEIGHT: u32 = 480;
const SCALE: u32 = 2;
const CURSOR: (i32, i32) = (138, 126);
const TOLERANCE: u8 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn scaled(self, scale: u32) -> Self {
        let s = scale as i32;
        Rect {
            x: self.x * s,
            y: self.y * s,
            w: self.w * s,
            h: self.h * s,
        }
    }

    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

impl Rgba {
    pub fn decode(bytes: &[u8]) -> Rgba {
        let image = Image::from_buffer(
            bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .expect("a PNG");
        let (width, height) = (image.width(), image.height());
        let rgba = image.try_into_dynamic().expect("convertible").into_rgba8();
        Rgba {
            width,
            height,
            pixels: rgba.into_raw(),
        }
    }

    pub fn load(path: &Path) -> Rgba {
        Rgba::decode(&std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
    }

    pub fn save(&self, path: &Path) {
        let image = Image::new(
            bevy::render::render_resource::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            self.pixels.clone(),
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image
            .try_into_dynamic()
            .expect("convertible")
            .save(path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }

    pub fn pixel(&self, x: i32, y: i32) -> [u8; 4] {
        let start = ((y as u32 * self.width + x as u32) * 4) as usize;
        self.pixels[start..start + 4].try_into().unwrap()
    }

    pub fn crop(&self, rect: Rect) -> Rgba {
        let mut pixels = Vec::with_capacity((rect.w * rect.h * 4) as usize);
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                pixels.extend_from_slice(&self.pixel(x, y));
            }
        }
        Rgba {
            width: rect.w as u32,
            height: rect.h as u32,
            pixels,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Mismatch {
    pub x: i32,
    pub y: i32,
    pub actual: [u8; 4],
    pub expected: [u8; 4],
}

/// Every unmasked pixel must match within `TOLERANCE` per channel; the diff image marks
/// mismatches in magenta over the expected picture.
pub fn compare(
    actual: &Rgba,
    expected: &Rgba,
    masks: &[Rect],
) -> Result<(), (Vec<Mismatch>, Rgba)> {
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height),
        "crop sizes"
    );
    let mut diff = expected.clone();
    let mut mismatches = Vec::new();
    for y in 0..expected.height as i32 {
        for x in 0..expected.width as i32 {
            if masks.iter().any(|mask| mask.contains(x, y)) {
                continue;
            }
            let (a, e) = (actual.pixel(x, y), expected.pixel(x, y));
            if a.iter().zip(e).any(|(a, e)| a.abs_diff(e) > TOLERANCE) {
                let start = ((y as u32 * diff.width + x as u32) * 4) as usize;
                diff.pixels[start..start + 4].copy_from_slice(&[255, 0, 255, 255]);
                mismatches.push(Mismatch {
                    x,
                    y,
                    actual: a,
                    expected: e,
                });
            }
        }
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err((mismatches, diff))
    }
}

pub fn gui_size(width: u32, height: u32, scale: u32) -> (i32, i32) {
    (width.div_ceil(scale) as i32, height.div_ceil(scale) as i32)
}

pub fn hotbar_rect(width: u32, height: u32, scale: u32) -> Rect {
    let (w, h) = gui_size(width, height, scale);
    Rect {
        x: w / 2 - 91,
        y: h - 22,
        w: 182,
        h: 22,
    }
}

pub fn inventory_rect(width: u32, height: u32, scale: u32) -> Rect {
    let (w, h) = gui_size(width, height, scale);
    Rect {
        x: (w - 176) / 2,
        y: (h - 166) / 2,
        w: 176,
        h: 166,
    }
}

fn title_mask(width: u32, height: u32, scale: u32) -> Rect {
    let inventory = inventory_rect(width, height, scale);
    Rect {
        x: inventory.x + 97,
        y: inventory.y + 6,
        w: 70,
        h: 9,
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gui")
}

fn check(shot: &Rgba, name: &str, rect: Rect, masks: &[Rect]) {
    let expected = Rgba::load(&fixtures().join(format!("{name}_{SCALE}.png")));
    let px = rect.scaled(SCALE);
    let local = |mask: Rect| Rect {
        x: mask.x - rect.x,
        y: mask.y - rect.y,
        ..mask
    };
    let masks: Vec<Rect> = masks.iter().map(|m| local(*m).scaled(SCALE)).collect();
    if let Err((mismatches, diff)) = compare(&shot.crop(px), &expected, &masks) {
        let path = fixtures().join(format!("{name}_{SCALE}.diff.png"));
        diff.save(&path);
        panic!(
            "{name}: {} pixels differ, first {:?}; diff written to {}",
            mismatches.len(),
            mismatches[0],
            path.display()
        );
    }
}

fn wait_for_png(dir: &Path, deadline: Duration) -> Option<PathBuf> {
    let started = Instant::now();
    while started.elapsed() < deadline {
        if let Some(png) = std::fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "png"))
        {
            std::thread::sleep(Duration::from_millis(500));
            return Some(png);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    None
}

/// `MCRS_GUI_PARITY=1 MCRS_GUI_PARITY_WORLD=<world folder> cargo test -p mcrs_minecraft_client
/// --test gui_parity -- --ignored`: the world folder holds the seeded player data the README describes.
#[test]
#[ignore]
fn hotbar_and_inventory_match_vanilla() {
    if std::env::var_os("MCRS_GUI_PARITY").is_none() {
        eprintln!("MCRS_GUI_PARITY is unset; skipping");
        return;
    }
    let world = std::env::var_os("MCRS_GUI_PARITY_WORLD")
        .expect("MCRS_GUI_PARITY_WORLD names the world folder");
    let settle = std::env::var("MCRS_GUI_PARITY_WAIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20u64);
    let shots = std::env::temp_dir().join(format!("mcrs-gui-parity-{}", std::process::id()));
    std::fs::create_dir_all(&shots).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_client"))
        .arg(&world)
        .env("MCRS_RESOLUTION", format!("{WIDTH}x{HEIGHT}"))
        .env("MCRS_GUI_SCALE", SCALE.to_string())
        .env("MCRS_SCREEN", "inventory")
        .env("MCRS_CURSOR", format!("{},{}", CURSOR.0, CURSOR.1))
        .env("MCRS_TIME", "6000")
        .env("MCRS_LOOK", "0,-90")
        .env("MCRS_SCREENSHOT_DIR", &shots)
        .stdout(Stdio::null())
        .spawn()
        .expect("the client binary starts");
    std::thread::sleep(Duration::from_secs(settle));
    std::fs::write(shots.join("capture"), b"").unwrap();
    let png = wait_for_png(&shots, Duration::from_secs(30));
    let _ = child.kill();
    let _ = child.wait();
    let png = png.expect("the client wrote a screenshot");
    let shot = Rgba::load(&png);
    assert_eq!(
        (shot.width, shot.height),
        (WIDTH, HEIGHT),
        "the window is {WIDTH}x{HEIGHT} physical pixels"
    );
    check(&shot, "hotbar", hotbar_rect(WIDTH, HEIGHT, SCALE), &[]);
    check(
        &shot,
        "inventory",
        inventory_rect(WIDTH, HEIGHT, SCALE),
        &[title_mask(WIDTH, HEIGHT, SCALE)],
    );
}

fn synthetic(width: u32, height: u32, fill: impl Fn(i32, i32) -> [u8; 4]) -> Rgba {
    let mut pixels = Vec::new();
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            pixels.extend_from_slice(&fill(x, y));
        }
    }
    Rgba {
        width,
        height,
        pixels,
    }
}

#[test]
fn the_rectangles_follow_the_ceiled_gui_size() {
    assert_eq!(gui_size(855, 480, 2), (428, 240));
    assert_eq!(
        hotbar_rect(854, 480, 2),
        Rect {
            x: 122,
            y: 218,
            w: 182,
            h: 22
        }
    );
    assert_eq!(
        inventory_rect(854, 480, 2),
        Rect {
            x: 125,
            y: 37,
            w: 176,
            h: 166
        }
    );
    assert_eq!(
        title_mask(854, 480, 2),
        Rect {
            x: 222,
            y: 43,
            w: 70,
            h: 9
        }
    );
    assert_eq!(
        hotbar_rect(854, 480, 2).scaled(2),
        Rect {
            x: 244,
            y: 436,
            w: 364,
            h: 44
        }
    );
}

#[test]
fn crop_takes_the_rectangle_row_by_row() {
    let image = synthetic(4, 3, |x, y| [x as u8, y as u8, 0, 255]);
    let crop = image.crop(Rect {
        x: 1,
        y: 1,
        w: 2,
        h: 2,
    });
    assert_eq!(crop.width, 2);
    assert_eq!(
        crop.pixels,
        vec![1, 1, 0, 255, 2, 1, 0, 255, 1, 2, 0, 255, 2, 2, 0, 255]
    );
}

#[test]
fn compare_accepts_a_one_step_difference_and_reports_more() {
    let expected = synthetic(3, 3, |x, y| [10 * x as u8, 10 * y as u8, 100, 255]);
    let same = expected.clone();
    assert!(compare(&same, &expected, &[]).is_ok());
    let mut nudged = expected.clone();
    nudged.pixels[0] += 1;
    assert!(compare(&nudged, &expected, &[]).is_ok());
    let mut off = expected.clone();
    off.pixels[4 * 4] += 2;
    let (mismatches, diff) = compare(&off, &expected, &[]).unwrap_err();
    assert_eq!(
        mismatches,
        vec![Mismatch {
            x: 1,
            y: 1,
            actual: [12, 10, 100, 255],
            expected: [10, 10, 100, 255]
        }]
    );
    assert_eq!(diff.pixel(1, 1), [255, 0, 255, 255]);
    assert_eq!(diff.pixel(0, 0), expected.pixel(0, 0));
}

#[test]
fn masked_pixels_are_not_compared() {
    let expected = synthetic(3, 3, |_, _| [0, 0, 0, 255]);
    let mut off = expected.clone();
    off.pixels[4 * 4] = 200;
    assert!(
        compare(
            &off,
            &expected,
            &[Rect {
                x: 1,
                y: 1,
                w: 1,
                h: 1
            }]
        )
        .is_ok()
    );
    assert!(
        compare(
            &off,
            &expected,
            &[Rect {
                x: 0,
                y: 0,
                w: 1,
                h: 1
            }]
        )
        .is_err()
    );
}

#[test]
fn a_diff_image_round_trips_through_png() {
    let image = synthetic(5, 4, |x, y| [x as u8 * 40, y as u8 * 60, 7, 255]);
    let path = std::env::temp_dir().join(format!(
        "mcrs-gui-parity-roundtrip-{}.png",
        std::process::id()
    ));
    image.save(&path);
    let back = Rgba::load(&path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(back, image);
}
