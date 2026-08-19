//! The time of day: what the terrain is lit by, what colour the sky is, where the sun and the moon
//! stand, and how far the stars have come out.
//!
//! Every number here is read off vanilla's own data rather than guessed. The base colours are the
//! attributes `dimension_type/overworld.json` sets, and the way each of them moves through the day
//! is the keyframed tracks of `timeline/day.json`. Those tracks are piecewise linear in ticks and
//! all wrap at the end of the day, so one interpolator serves every one of them.
//!
//! What this module produces is a single uniform; `sky.wgsl` draws the sky from it and the terrain
//! shader lights the world from it, and neither has any other notion of what hour it is.

use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

use crate::model;
use crate::render::{Atlas, Clouds, Sky};

/// Ticks in a day, from the period of the overworld clock.
const DAY: f32 = 24000.0;

/// The moon runs on a period of its own, eight days long, which is what gives it phases. The order
/// is the one `timeline/moon.json` keyframes.
const MOON_PHASES: [&str; 8] = [
    "full_moon",
    "waning_gibbous",
    "third_quarter",
    "waning_crescent",
    "new_moon",
    "waxing_crescent",
    "first_quarter",
    "waxing_gibbous",
];

/// How fast a held key scrubs, in ticks a second: a whole day in six seconds.
const SCRUB: f32 = DAY / 6.0;

// The overworld's own attributes. The tracks below scale these rather than replace them.
const BASE_SKY: Vec3 = rgb(0x78a7ff);
const BASE_FOG: Vec3 = rgb(0xc0d8ff);
const AMBIENT: Vec3 = rgb(0x0a0a0a);

/// `visual/block_light_tint`, the colour a torch reads as at its dimmest, and the factor block
/// light is scaled by. Neither moves with the hour: a torch burns the same at midnight.
const BLOCK_LIGHT_TINT: Vec3 = rgb(0xffd88c);
const BLOCK_LIGHT_FACTOR: f32 = 1.4;

/// `visual/cloud_height`: where the underside of the cloud layer sits.
const CLOUD_HEIGHT: f32 = 192.33;

/// How fast the field drifts, in blocks a second. Vanilla moves it three hundredths of a block a
/// tick, and ties that to the world's clock; there is no world here, so it runs off the wall clock
/// the animated sprites already run off.
const CLOUD_SPEED: f32 = 0.6;

/// The offset vanilla starts the field at in z.
const CLOUD_ORIGIN_Z: f32 = 3.96;

/// `visual/cloud_fog_end_distance`, capped by the cloud range option — both of which land here.
/// The clouds fade out linearly over this and are gone beyond it, which is also where the march
/// through them stops.
const CLOUD_FADE: f32 = 2048.0;

/// `visual/cloud_color`, which carries the alpha the layer is drawn at.
const CLOUD_COLOR: Vec4 = argb(0xccffffff);

/// The cloud half of `visual/cloud_color`'s track: white by day, near black by night.
const CLOUD_TINT: [(f32, Vec3); 4] = [
    (133.0, Vec3::ONE),
    (11867.0, Vec3::ONE),
    (13670.0, rgb(0x191926)),
    (22330.0, rgb(0x191926)),
];

/// How far out the sky has faded entirely into the haze: `visual/sky_fog_end_distance`, which
/// vanilla caps by the render distance. Nothing is culled by distance here, so the attribute stands.
const SKY_FOG_END: f32 = 512.0;

/// Sea level, which is the height vanilla calls the horizon: from below it the void is covered by a
/// dark disc rather than left showing the sky.
const HORIZON: f32 = 63.0;

/// There is no weather here, so nothing ever dims the sun and the moon. Vanilla drives this from
/// the rain level and the sky shader still reads it, so the two stay the same shape.
const RAIN_BRIGHTNESS: f32 = 1.0;

/// `visual/sky_light_factor`: how much of the sky's light the hour lets through.
const SKY_LIGHT_FACTOR: [(f32, f32); 4] =
    [(730.0, 1.0), (11270.0, 1.0), (13140.0, 0.24), (22860.0, 0.24)];

/// `visual/sky_color`, a multiplier over the dimension's own colour rather than a colour. Vanilla
/// keyframes it white fading to black, so one number says what all three channels do.
const SKY_COLOR: [(f32, f32); 4] = [(133.0, 1.0), (11867.0, 1.0), (13670.0, 0.0), (22330.0, 0.0)];

/// `visual/fog_color`, the same idea but not grey: the night haze keeps a little blue in it.
const FOG_COLOR: [(f32, Vec3); 4] = [
    (133.0, Vec3::ONE),
    (11867.0, Vec3::ONE),
    (13670.0, rgb(0x0c0c16)),
    (22330.0, rgb(0x161616)),
];

/// `visual/sky_light_color`: white by day, `#7a7aff` by night.
const SKY_LIGHT_COLOR: [(f32, Vec3); 4] = [
    (730.0, Vec3::ONE),
    (11270.0, Vec3::ONE),
    (13140.0, rgb(0x7a7aff)),
    (22860.0, rgb(0x7a7aff)),
];

/// `visual/star_brightness`. The stars are out all night and fade through both twilights.
const STAR_BRIGHTNESS: [(f32, f32); 12] = [
    (92.0, 0.037),
    (627.0, 0.0),
    (11373.0, 0.0),
    (11732.0, 0.016),
    (11959.0, 0.044),
    (12399.0, 0.143),
    (12729.0, 0.258),
    (13228.0, 0.5),
    (22772.0, 0.5),
    (23032.0, 0.364),
    (23356.0, 0.225),
    (23758.0, 0.101),
];

/// `visual/sunrise_sunset_color`, as `0xAARRGGBB`. The alpha carries the band in and out, so it is
/// zero through the middle of the day and the middle of the night; the colour swings from yellow
/// through orange to red as the sun goes down, and back the other way as it comes up.
const SUNRISE: [(f32, Vec4); 32] = [
    (71.0, argb(0x5fefa333)),
    (310.0, argb(0x29f5ba33)),
    (565.0, argb(0x06fbd433)),
    (730.0, argb(0x00ffe533)),
    (11270.0, argb(0x00ffe533)),
    (11397.0, argb(0x04fcd833)),
    (11522.0, argb(0x0ff9cb33)),
    (11690.0, argb(0x29f5ba33)),
    (11929.0, argb(0x5fefa333)),
    (12243.0, argb(0xb1e78733)),
    (12358.0, argb(0xcce47e33)),
    (12512.0, argb(0xe9e07233)),
    (12613.0, argb(0xf6dd6b33)),
    (12732.0, argb(0xfeda6333)),
    (12841.0, argb(0xfed75c33)),
    (13035.0, argb(0xecd25133)),
    (13252.0, argb(0xc1cc4733)),
    (13775.0, argb(0x36be3733)),
    (13888.0, argb(0x1fbb3533)),
    (14039.0, argb(0x09b73333)),
    (14192.0, argb(0x00b33333)),
    (21807.0, argb(0x00b23333)),
    (21961.0, argb(0x09b73333)),
    (22112.0, argb(0x1fbb3533)),
    (22225.0, argb(0x36be3733)),
    (22748.0, argb(0xc1cc4733)),
    (22965.0, argb(0xecd25133)),
    (23159.0, argb(0xfed75c33)),
    (23272.0, argb(0xfeda6333)),
    (23488.0, argb(0xe9e07233)),
    (23642.0, argb(0xcce47e33)),
    (23757.0, argb(0xb1e78733)),
];

const fn rgb(hex: u32) -> Vec3 {
    Vec3::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    )
}

const fn argb(hex: u32) -> Vec4 {
    Vec4::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        ((hex >> 24) & 0xff) as f32 / 255.0,
    )
}

/// Where the clock stands, in ticks. Kept over eight days rather than one so the moon works
/// through its phases as the hour is scrubbed forward.
#[derive(Resource)]
pub struct TimeOfDay {
    pub ticks: f32,
}

impl Default for TimeOfDay {
    /// Noon, unless `ANVIL_TIME=<ticks>` pins the clock somewhere else — which is what lets a
    /// night frame be shot from a terminal with nobody at the window to hold a key down.
    fn default() -> Self {
        let ticks = std::env::var("ANVIL_TIME")
            .ok()
            .and_then(|ticks| ticks.trim().parse().ok())
            .unwrap_or(6000.0);
        Self { ticks }
    }
}

impl TimeOfDay {
    /// The hour as a clock reads it: tick zero is six in the morning.
    pub fn clock(&self) -> (u32, u32) {
        let minutes = (self.ticks.rem_euclid(DAY) / DAY * 1440.0 + 360.0) % 1440.0;
        (minutes as u32 / 60, minutes as u32 % 60)
    }
}

pub struct DayCyclePlugin;

impl Plugin for DayCyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeOfDay>()
            .add_systems(Update, (scrub, apply).chain())
            .add_systems(Update, toggle_clouds);
    }
}

impl Default for Sky {
    fn default() -> Self {
        sky_at(TimeOfDay::default().ticks, f32::MAX, 0.0)
    }
}

/// The cloud field: one texel a cell, open where the sky shows through.
pub fn clouds() -> Result<Atlas, String> {
    sprites(&["environment/clouds".to_string()])
}

/// The sun and the eight moon phases, decoded into one square array for the sky shader to sample.
pub fn celestials() -> Result<Atlas, String> {
    let sun = std::iter::once("environment/celestial/sun".to_string());
    let moon = MOON_PHASES.map(|phase| format!("environment/celestial/moon/{phase}"));
    sprites(&sun.chain(moon).collect::<Vec<_>>())
}

/// Decodes square sprites of one size into the layers of a single array.
fn sprites(ids: &[String]) -> Result<Atlas, String> {
    let mut pixels = Vec::new();
    let mut side = 0;
    for id in ids {
        let path = model::resource_path(id, "textures", "png");
        let bytes =
            std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let image = Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
        if side != 0 && image.width() != side {
            return Err(format!(
                "{} is {}x{} where the sprites before it were {side}x{side}",
                path.display(),
                image.width(),
                image.height(),
            ));
        }
        if image.width() != image.height() {
            return Err(format!("{} is not square", path.display()));
        }
        side = image.width();
        pixels.extend(
            image
                .data
                .ok_or_else(|| format!("{} decoded without pixel data", path.display()))?,
        );
    }
    Ok(Atlas {
        size: side,
        layers: ids.len() as u32,
        // One level: none of these is ever seen small enough for a mip to be the right sample, and
        // filtering them at all is what turned the sun into a smear.
        mips: vec![pixels],
    })
}

/// One keyframed track, read at a tick. The last keyframe runs into the first one a day later,
/// which is how a track that ends before midnight still has a value at one in the morning.
fn track<T>(keys: &[(f32, T)], ticks: f32) -> T
where
    T: Copy + std::ops::Add<T, Output = T> + std::ops::Mul<f32, Output = T>,
{
    let mut at = ticks.rem_euclid(DAY);
    if at < keys[0].0 {
        at += DAY;
    }
    for (index, &(start, value)) in keys.iter().enumerate() {
        let (mut end, next) = keys[(index + 1) % keys.len()];
        if index + 1 == keys.len() {
            end += DAY;
        }
        if at <= end {
            let fraction = (at - start) / (end - start);
            return value * (1.0 - fraction) + next * fraction;
        }
    }
    keys[0].1
}

/// Everything downstream works in the space its textures are sampled in, which is linear, so a
/// colour written down the way a resource pack writes it has to be converted. Doing it here rather
/// than in the shader is also what puts the arithmetic where vanilla's is: a factor applied to a
/// linear colour and then encoded is the factor vanilla applies to the colour already encoded.
fn linear(color: Vec3) -> Vec3 {
    let color = LinearRgba::from(Color::srgb(color.x, color.y, color.z));
    Vec3::new(color.red, color.green, color.blue)
}

/// Where the sun stands, in radians about the world's east-west axis. The timeline eases this with
/// a cubic bezier; the closed form below is the curve that bezier was fitted to — flat through the
/// middle of the day and of the night, quick through the two twilights.
fn sun_angle(ticks: f32) -> f32 {
    let day = (ticks / DAY - 0.25).rem_euclid(1.0);
    let eased = 0.5 - (day * PI).cos() * 0.5;
    (day * 2.0 + eased) / 3.0 * TAU
}

fn sky_at(ticks: f32, camera_height: f32, drift: f32) -> Sky {
    let sun = sun_angle(ticks);
    let sunrise = track(&SUNRISE, ticks);
    // A star is drawn as its own brightness times itself, because the pipeline both writes the
    // value and blends by it. Vanilla squares a number that is already encoded, so ours is
    // converted before it is squared rather than after, or the night would come out far brighter.
    let stars = linear(Vec3::splat(track(&STAR_BRIGHTNESS, ticks))).x.sqrt();
    Sky {
        sky_light: linear(track(&SKY_LIGHT_COLOR, ticks))
            .extend(track(&SKY_LIGHT_FACTOR, ticks))
            .into(),
        block_light: linear(BLOCK_LIGHT_TINT).extend(BLOCK_LIGHT_FACTOR).into(),
        ambient: linear(AMBIENT).extend(0.0).into(),
        disc: linear(BASE_SKY * track(&SKY_COLOR, ticks))
            .extend(f32::from(camera_height < HORIZON))
            .into(),
        sunrise: linear(sunrise.truncate()).extend(sunrise.w).into(),
        // Vanilla keyframes the three angles apart but says the same thing with them: the moon is
        // opposite the sun, and the stars turn with it.
        angles: [sun, sun + PI, sun, stars],
        moon: [
            1.0 + (ticks / DAY).floor().rem_euclid(MOON_PHASES.len() as f32),
            RAIN_BRIGHTNESS,
            0.0,
            0.0,
        ],
        fog: linear(fog(ticks)).extend(SKY_FOG_END).into(),
        cloud_color: linear(CLOUD_COLOR.truncate() * track(&CLOUD_TINT, ticks))
            .extend(CLOUD_COLOR.w)
            .into(),
        cloud: [CLOUD_HEIGHT, drift, CLOUD_ORIGIN_Z, CLOUD_FADE],
    }
}

/// The haze the world fades into. Below the horizon it is all that shows, so it is also what the
/// frame is cleared to, and the disc fading into exactly this is what hides the disc's own rim.
fn fog(ticks: f32) -> Vec3 {
    BASE_FOG * track(&FOG_COLOR, ticks)
}

/// Hold `+` or `-` to run the clock forward or back.
fn scrub(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut day: ResMut<TimeOfDay>) {
    let forward = keys.any_pressed([KeyCode::Equal, KeyCode::NumpadAdd]);
    let back = keys.any_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]);
    let direction = f32::from(forward) - f32::from(back);
    if direction != 0.0 {
        day.ticks = (day.ticks + direction * SCRUB * time.delta_secs())
            .rem_euclid(DAY * MOON_PHASES.len() as f32);
    }
}

/// Press F9 to take the cloud layer out of the frame.
fn toggle_clouds(keys: Res<ButtonInput<KeyCode>>, mut clouds: ResMut<Clouds>) {
    if keys.just_pressed(KeyCode::F9) {
        clouds.0 = !clouds.0;
    }
}

fn apply(
    day: Res<TimeOfDay>,
    time: Res<Time>,
    camera: Single<&Transform, With<Camera3d>>,
    mut sky: ResMut<Sky>,
    mut clear: ResMut<ClearColor>,
) {
    *sky = sky_at(day.ticks, camera.translation.y, time.elapsed_secs() * CLOUD_SPEED);
    let haze = fog(day.ticks);
    clear.0 = Color::srgb(haze.x, haze.y, haze.z);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_holds_its_plateaus_and_wraps_through_midnight() {
        assert_eq!(track(&SKY_LIGHT_FACTOR, 6000.0), 1.0, "noon is full daylight");
        assert_eq!(track(&SKY_LIGHT_FACTOR, 18000.0), 0.24, "midnight is not");
        // Between the last keyframe of the day and the first of the next one, which is the segment
        // that only exists because the track wraps.
        let dawn = track(&SKY_LIGHT_FACTOR, 23000.0);
        assert!(dawn > 0.24 && dawn < 1.0, "dawn is between the two, not {dawn}");
        assert_eq!(track(&SKY_LIGHT_FACTOR, -1000.0), track(&SKY_LIGHT_FACTOR, 23000.0));
    }

    #[test]
    fn the_twilight_band_only_shows_around_the_two_twilights() {
        assert_eq!(track(&SUNRISE, 6000.0).w, 0.0, "no band at noon");
        assert_eq!(track(&SUNRISE, 18000.0).w, 0.0, "none at midnight either");
        assert!(track(&SUNRISE, 12841.0).w > 0.9, "dusk is when it is strongest");
        assert!(track(&SUNRISE, 23159.0).w > 0.9, "and dawn the other side of the night");
    }

    #[test]
    fn the_sun_is_overhead_at_noon_and_sets_in_the_west() {
        let overhead = Quat::from_rotation_y(-PI / 2.0)
            * Quat::from_rotation_x(sun_angle(6000.0))
            * Vec3::Y;
        assert!(overhead.abs_diff_eq(Vec3::Y, 1e-4), "noon puts the sun at {overhead}");

        // Dusk is eased, so the sun is still just above the horizon when the day's own count runs
        // out; it crosses a little before the hour the timeline calls night.
        let setting = Quat::from_rotation_y(-PI / 2.0)
            * Quat::from_rotation_x(sun_angle(12000.0))
            * Vec3::Y;
        assert!(setting.x < -0.9 && setting.y > 0.0, "dusk puts the sun at {setting}");

        let midnight = Quat::from_rotation_y(-PI / 2.0)
            * Quat::from_rotation_x(sun_angle(18000.0))
            * Vec3::Y;
        assert!(midnight.abs_diff_eq(Vec3::NEG_Y, 1e-4), "midnight puts the sun at {midnight}");
    }

    #[test]
    fn the_moon_works_through_its_phases_and_returns_to_the_first() {
        let layer = |day: f32| sky_at(day * DAY + 15000.0, 0.0, 0.0).moon[0];
        assert_eq!(layer(0.0), 1.0, "the first night is the full moon");
        assert_eq!(layer(3.0), 4.0);
        assert_eq!(layer(8.0), 1.0, "and eight nights on it is full again");
    }

    #[test]
    fn the_dark_disc_is_only_drawn_from_under_the_horizon() {
        assert_eq!(sky_at(6000.0, HORIZON + 1.0, 0.0).disc[3], 0.0);
        assert_eq!(sky_at(6000.0, HORIZON - 1.0, 0.0).disc[3], 1.0);
    }

    #[test]
    fn the_clock_reads_from_six_in_the_morning() {
        assert_eq!(TimeOfDay { ticks: 0.0 }.clock(), (6, 0));
        assert_eq!(TimeOfDay { ticks: 6000.0 }.clock(), (12, 0));
        assert_eq!(TimeOfDay { ticks: 18000.0 }.clock(), (0, 0));
    }
}
