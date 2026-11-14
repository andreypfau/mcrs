use std::ops::{Add, Mul};

use bevy::math::{IRect, Rect, Vec2, Vec4};

/// A quad drawn from the GUI atlas: its rectangle in GUI units, and the texture coordinate and the
/// ARGB colour of each corner, from the top left corner clockwise. A glyph's rectangle may lie
/// between pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AtlasQuad {
    pub rect: Rect,
    pub uv: [[f32; 2]; 4],
    pub colors: [u32; 4],
}

impl AtlasQuad {
    /// The part of the quad inside `clip`, with the texture coordinates and the colours of its
    /// new corners, or `None` when no part of it is inside.
    #[must_use]
    pub fn clip(self, clip: IRect) -> Option<Self> {
        let cut = self.rect.intersect(clip.as_rect());
        if cut.is_empty() {
            return None;
        }
        if cut == self.rect {
            return Some(self);
        }
        let size = self.rect.size();
        let corners = [
            cut.min,
            Vec2::new(cut.max.x, cut.min.y),
            cut.max,
            Vec2::new(cut.min.x, cut.max.y),
        ]
        .map(|corner| (corner - self.rect.min) / size);
        let uv = self.uv.map(Vec2::from);
        let colors = self.colors.map(channels);
        Some(Self {
            rect: cut,
            uv: corners.map(|at| bilinear(uv, at).to_array()),
            colors: corners.map(|at| argb(bilinear(colors, at))),
        })
    }
}

/// The value at the fraction `at` of a quad from the values at its corners, from the top left
/// corner clockwise.
fn bilinear<T: Copy + Add<Output = T> + Mul<f32, Output = T>>(corners: [T; 4], at: Vec2) -> T {
    let [top_left, top_right, bottom_right, bottom_left] = corners;
    let top = top_left * (1.0 - at.x) + top_right * at.x;
    let bottom = bottom_left * (1.0 - at.x) + bottom_right * at.x;
    top * (1.0 - at.y) + bottom * at.y
}

fn channels(argb: u32) -> Vec4 {
    Vec4::from_array(argb.to_be_bytes().map(f32::from))
}

fn argb(channels: Vec4) -> u32 {
    u32::from_be_bytes(channels.round().to_array().map(|channel| channel as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quad_across_an_edge_of_the_clip_keeps_its_part_inside() {
        let (top, bottom) = (0xFF000000, 0xFF000080);
        let quad = |rect: [i32; 4], uv: [f32; 4], colors: [u32; 2]| AtlasQuad {
            rect: IRect::new(rect[0], rect[1], rect[2], rect[3]).as_rect(),
            uv: [
                [uv[0], uv[1]],
                [uv[2], uv[1]],
                [uv[2], uv[3]],
                [uv[0], uv[3]],
            ],
            colors: [colors[0], colors[0], colors[1], colors[1]],
        };
        let whole = quad([0, 0, 16, 8], [32.0, 48.0, 48.0, 56.0], [top, bottom]);
        for (name, clip, expected) in [
            (
                "across the left edge",
                [4, -10, 100, 100],
                Some(quad([4, 0, 16, 8], [36.0, 48.0, 48.0, 56.0], [top, bottom])),
            ),
            (
                "across the top edge",
                [-10, 2, 100, 100],
                Some(quad(
                    [0, 2, 16, 8],
                    [32.0, 50.0, 48.0, 56.0],
                    [0xFF000020, bottom],
                )),
            ),
            (
                "across the right edge",
                [-10, -10, 10, 100],
                Some(quad([0, 0, 10, 8], [32.0, 48.0, 42.0, 56.0], [top, bottom])),
            ),
            (
                "across the bottom edge",
                [-10, -10, 100, 6],
                Some(quad(
                    [0, 0, 16, 6],
                    [32.0, 48.0, 48.0, 54.0],
                    [top, 0xFF000060],
                )),
            ),
            (
                "across every edge",
                [2, 1, 14, 7],
                Some(quad(
                    [2, 1, 14, 7],
                    [34.0, 49.0, 46.0, 55.0],
                    [0xFF000010, 0xFF000070],
                )),
            ),
            ("inside", [0, 0, 16, 8], Some(whole)),
            ("outside", [20, 0, 30, 8], None),
            ("touching the right edge", [16, 0, 30, 8], None),
        ] {
            let clip = IRect::new(clip[0], clip[1], clip[2], clip[3]);
            assert_eq!(whole.clip(clip), expected, "{name}");
        }
    }
}
