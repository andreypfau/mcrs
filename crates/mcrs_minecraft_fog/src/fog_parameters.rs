use crate::{FogInputs, FogRamp};

const BLOCKS_PER_CHUNK: f32 = 16.0;
const SUNRISE_MIN_RENDER_DISTANCE: u8 = 4;
const RAIN_DARKNESS: [f32; 3] = [0.5, 0.5, 0.4];
const THUNDER_DARKNESS: f32 = 0.5;
const SKY_TERM_CHUNKS: f32 = 32.0;
const SKY_TERM_LOW_END: f32 = 0.25;
const SKY_TERM_EXPONENT: f64 = 0.25;
// CFOG-DEV-003: 16 blocks times 32 chunks, the largest render distance.
const MAX_ENVIRONMENTAL_FOG_END: f32 = 512.0;

/// The fog of one frame, which each draw of the world in that frame reads. A distance is in
/// blocks.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FogParameters {
    /// The colour that the fog mixes into a draw, three image values in `[0, 1]`.
    pub color: [f32; 3],
    /// The environmental fog, over the spherical distance of a point.
    pub environmental: FogRamp,
    /// The render-distance fog, over the cylindrical distance of a point.
    pub render_distance: FogRamp,
    /// The distance at which the sky takes the full fog.
    pub sky_end: f32,
    /// The distance at which the clouds take the full fog.
    pub cloud_end: f32,
}

impl FogParameters {
    /// The fog of the frame that `inputs` describe. The environmental fog takes no rain: it is
    /// the fog start and end distances, or no fog when the end is beyond 512 blocks.
    #[must_use]
    pub fn from_inputs(inputs: &FogInputs) -> Self {
        let render_end = BLOCKS_PER_CHUNK * f32::from(inputs.render_distance);
        let render_fog_end = inputs.render_fog_end;
        let render_length = (render_fog_end / 10.0).clamp(4.0, 64.0);
        let cloud_end = BLOCKS_PER_CHUNK * f32::from(inputs.cloud_distance);
        Self {
            color: color(inputs),
            environmental: if inputs.fog_end_distance > MAX_ENVIRONMENTAL_FOG_END {
                FogRamp::NONE
            } else {
                FogRamp {
                    start: inputs.fog_start_distance,
                    end: inputs.fog_end_distance,
                }
            },
            render_distance: FogRamp {
                start: render_fog_end - render_length,
                end: render_fog_end,
            },
            sky_end: render_end.min(inputs.sky_fog_end_distance),
            cloud_end: cloud_end.min(inputs.cloud_fog_end_distance),
        }
    }
}

fn color(inputs: &FogInputs) -> [f32; 3] {
    let mut color = inputs.fog_color;
    if inputs.render_distance >= SUNRISE_MIN_RENDER_DISTANCE {
        let sun_east: f32 = if libm::sinf(inputs.sun_angle.to_radians()) > 0.0 {
            -1.0
        } else {
            1.0
        };
        let toward_sun = sun_east * inputs.view_east;
        if toward_sun > 0.0 && inputs.sunrise_alpha > 0.0 {
            color = lerp(
                toward_sun * inputs.sunrise_alpha,
                color,
                inputs.sunrise_color,
            );
        }
    }

    let mut sky = inputs.sky_color;
    if inputs.rain_level > 0.0 {
        sky = darken(
            sky,
            RAIN_DARKNESS.map(|darkness| 1.0 - darkness * inputs.rain_level),
        );
    }
    if inputs.thunder_level > 0.0 {
        sky = darken(sky, [1.0 - THUNDER_DARKNESS * inputs.thunder_level; 3]);
    }

    let chunks =
        (inputs.sky_fog_end_distance / BLOCKS_PER_CHUNK).min(f32::from(inputs.render_distance));
    let progress = (chunks / SKY_TERM_CHUNKS).clamp(0.0, 1.0);
    let base = SKY_TERM_LOW_END + progress * (1.0 - SKY_TERM_LOW_END);
    let sky_share = 1.0 - libm::pow(f64::from(base), SKY_TERM_EXPONENT) as f32;
    lerp(sky_share, color, sky)
}

fn lerp(amount: f32, from: [f32; 3], to: [f32; 3]) -> [f32; 3] {
    let mut color = from;
    for (channel, target) in color.iter_mut().zip(to) {
        *channel += amount * (target - *channel);
    }
    color
}

fn darken(color: [f32; 3], factors: [f32; 3]) -> [f32; 3] {
    let mut darker = color;
    for (channel, factor) in darker.iter_mut().zip(factors) {
        *channel = (*channel * factor).clamp(0.0, 1.0);
    }
    darker
}
