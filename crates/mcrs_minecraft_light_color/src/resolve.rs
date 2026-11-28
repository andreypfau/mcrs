use mcrs_voxel::BlockPos;

use crate::colors::LightColors;
use crate::region::{Palette, Region};

pub struct Lanes {
    pub bytes: usize,
    pub levels: Box<[u8]>,
}

impl Lanes {
    #[inline]
    pub fn level(&self, cell: usize, lane: usize) -> Option<u8> {
        self.levels.get(cell * self.bytes + lane).copied()
    }
}

pub fn light_weight(level: u8) -> f32 {
    let f = level as f32 / 15.0;
    f / (4.0 - 3.0 * f)
}

#[expect(
    clippy::excessive_nesting,
    reason = "channel sums inline, lane by lane"
)]
pub fn resolve(
    region: &Region,
    lanes: &Lanes,
    palette: &Palette,
    colours: &LightColors,
    output_min: BlockPos,
    output_size: i32,
) -> Box<[[u8; 4]]> {
    let known: Vec<Option<[f32; 3]>> = palette
        .types
        .iter()
        .map(|&t| colours.rgb(t).map(|c| c.map(f32::from)))
        .collect();
    let offset = output_min - region.min.as_ivec3();

    let mut out = Vec::with_capacity((output_size * output_size * output_size) as usize);
    for y in 0..output_size {
        for z in 0..output_size {
            for x in 0..output_size {
                let cell = region.index(offset.x + x, offset.y + y, offset.z + z);
                let mut rgb = [0f32; 3];
                let (mut total, mut default) = (0f32, 0f32);
                for (lane, colour) in known.iter().enumerate() {
                    let weight = lanes.level(cell, lane).map_or(0.0, light_weight);
                    total += weight;
                    match colour {
                        Some(colour) => {
                            for (sum, channel) in rgb.iter_mut().zip(colour) {
                                *sum += channel * weight;
                            }
                        }
                        None => default += weight,
                    }
                }
                out.push(if total == 0.0 {
                    [0, 0, 0, 255]
                } else {
                    let [r, g, b] = rgb.map(|sum| (sum / total).round() as u8);
                    [r, g, b, (default * 255.0 / total).round() as u8]
                });
            }
        }
    }
    out.into_boxed_slice()
}
