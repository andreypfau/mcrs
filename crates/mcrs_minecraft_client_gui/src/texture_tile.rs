use bevy::math::{IRect, IVec2, Rect};

use crate::gui_texture_id::GuiTextureId;
use crate::scene::{GuiAtlasData, GuiQuad};

impl GuiAtlasData {
    /// Repeats `texture` over `rect` in tiles of `size` GUI units, each the full image, on a grid
    /// through `start`. A tile across an edge of `rect` shows only its part inside.
    pub fn tile(
        &self,
        texture: GuiTextureId,
        rect: IRect,
        start: IVec2,
        size: IVec2,
        out: &mut Vec<GuiQuad>,
    ) {
        let texels = self.textures.get(texture as usize).copied();
        tile(texels.unwrap_or_default().as_rect(), rect, start, size, out);
    }
}

fn tile(texels: Rect, rect: IRect, start: IVec2, size: IVec2, out: &mut Vec<GuiQuad>) {
    if size.min_element() <= 0 || rect.is_empty() {
        return;
    }
    let first = start + (rect.min - start).div_euclid(size) * size;
    let scale = texels.size() / size.as_vec2();
    for y in (first.y..rect.max.y).step_by(size.y as usize) {
        for x in (first.x..rect.max.x).step_by(size.x as usize) {
            let at = IVec2::new(x, y);
            let cut = IRect::from_corners(at, at + size).intersect(rect);
            let texel = |corner: IVec2| texels.min + (corner - at).as_vec2() * scale;
            out.push(GuiQuad::Texels {
                rect: cut,
                texels: Rect::from_corners(texel(cut.min), texel(cut.max)),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Quad = ([i32; 4], [f32; 4]);

    #[test]
    fn the_tiles_on_the_grid_through_the_start_are_cut_at_each_edge_of_the_rectangle() {
        let cases: [(&str, Rect, [i32; 4], IVec2, IVec2, Vec<Quad>); 3] = [
            (
                "an image of 16 pixels in tiles of 32 from the top left corner",
                Rect::new(32.0, 0.0, 48.0, 16.0),
                [0, 0, 40, 20],
                IVec2::ZERO,
                IVec2::splat(32),
                vec![
                    ([0, 0, 32, 20], [32.0, 0.0, 48.0, 10.0]),
                    ([32, 0, 40, 20], [32.0, 0.0, 36.0, 10.0]),
                ],
            ),
            (
                "a separator across from its left edge",
                Rect::new(0.0, 64.0, 32.0, 66.0),
                [10, 22, 80, 24],
                IVec2::new(10, 22),
                IVec2::new(32, 2),
                vec![
                    ([10, 22, 42, 24], [0.0, 64.0, 32.0, 66.0]),
                    ([42, 22, 74, 24], [0.0, 64.0, 32.0, 66.0]),
                    ([74, 22, 80, 24], [0.0, 64.0, 6.0, 66.0]),
                ],
            ),
            (
                "a grid that starts inside the rectangle",
                Rect::new(0.0, 0.0, 16.0, 16.0),
                [10, 10, 30, 20],
                IVec2::new(14, 14),
                IVec2::splat(16),
                vec![
                    ([10, 10, 14, 14], [12.0, 12.0, 16.0, 16.0]),
                    ([14, 10, 30, 14], [0.0, 12.0, 16.0, 16.0]),
                    ([10, 14, 14, 20], [12.0, 0.0, 16.0, 6.0]),
                    ([14, 14, 30, 20], [0.0, 0.0, 16.0, 6.0]),
                ],
            ),
        ];
        for (name, texels, rect, start, size, expected) in cases {
            let mut out = Vec::new();
            let rect = IRect::new(rect[0], rect[1], rect[2], rect[3]);
            tile(texels, rect, start, size, &mut out);
            let quads: Vec<Quad> = out
                .into_iter()
                .filter_map(|quad| match quad {
                    GuiQuad::Texels { rect, texels } => Some((
                        [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
                        [texels.min.x, texels.min.y, texels.max.x, texels.max.y],
                    )),
                    _ => None,
                })
                .collect();
            assert_eq!(quads, expected, "{name}");
        }
    }
}
