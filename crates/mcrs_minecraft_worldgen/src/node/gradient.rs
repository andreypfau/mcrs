use crate::jmath::{floor_div, floor_mod};
use crate::volume::{Axis, Volume};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum Tiling {
    #[default]
    ClampToEdge,
    Repeat,
    MirroredRepeat,
}

pub type TilingMode = Tiling;

#[derive(Clone, Copy, Debug)]
pub struct GradientParams {
    pub axis: Axis,
    pub tiling: Tiling,
    pub from: f32,
    pub to: f32,
    pub from_value: f32,
    pub to_value: f32,
}

impl GradientParams {
    pub fn eval(&self, out: &mut [f32], ext: &Volume) {
        let from_coordinate = self.from as i32;
        let to_coordinate = self.to as i32;
        let range = to_coordinate - from_coordinate;
        debug_assert_ne!(range, 0, "from_coordinate cannot be equal to to_coordinate");
        let factor = (self.to_value - self.from_value) / range as f32;
        let base = self.from_value;

        match self.tiling {
            Tiling::ClampToEdge => {
                let lo = from_coordinate.min(to_coordinate);
                let hi = from_coordinate.max(to_coordinate);
                // The clamp uses the sorted pair while the offset uses the
                // original from_coordinate; a descending gradient needs both.
                self.fill(out, ext, |c| {
                    base + (c.clamp(lo, hi) - from_coordinate) as f32 * factor
                })
            }
            Tiling::Repeat => self.fill(out, ext, |c| {
                base + floor_mod(c - from_coordinate, range) as f32 * factor
            }),
            Tiling::MirroredRepeat => self.fill(out, ext, |c| {
                let relative = c - from_coordinate;
                let tile = floor_div(relative, range);
                let local = relative - tile * range;
                // Two's-complement parity, which stays correct for a negative
                // tile; `tile % 2 == 0` does not.
                let offset = if tile & 1 == 0 { local } else { range - local };
                base + offset as f32 * factor
            }),
        }
    }

    /// The stratum of a gradient is exactly its own axis, so the extent has one
    /// sample on the other two and the buffer walks that axis end to end.
    fn fill(&self, out: &mut [f32], ext: &Volume, compute: impl Fn(i32) -> f32) {
        let coordinate: fn(&Volume, i32) -> i32 = match self.axis {
            Axis::X => Volume::block_x,
            Axis::Y => Volume::block_y,
            Axis::Z => Volume::block_z,
        };
        for (i, o) in out.iter_mut().enumerate() {
            *o = compute(coordinate(ext, i as i32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::IVec3;

    fn gradient(
        axis: Axis,
        tiling: Tiling,
        from: f32,
        to: f32,
        fv: f32,
        tv: f32,
    ) -> GradientParams {
        GradientParams {
            axis,
            tiling,
            from,
            to,
            from_value: fv,
            to_value: tv,
        }
    }

    fn run(g: &GradientParams, ext: Volume) -> Vec<f32> {
        let mut out = vec![0.0; ext.len()];
        g.eval(&mut out, &ext);
        out
    }

    fn one(g: &GradientParams, x: i32, y: i32, z: i32) -> f32 {
        run(g, Volume::point(IVec3::new(x, y, z)))[0]
    }

    #[test]
    fn the_run_walks_the_extent_step() {
        let g = gradient(Axis::Y, Tiling::ClampToEdge, 0.0, 8.0, 0.0, 8.0);
        let ext = Volume::new(
            IVec3::new(1, 6, 1),
            IVec3::new(0, -2, 0),
            IVec3::new(1, 2, 1),
        );
        assert_eq!(run(&g, ext), vec![0.0, 0.0, 2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn a_horizontal_gradient_walks_its_own_axis() {
        let g = gradient(Axis::X, Tiling::ClampToEdge, -8.0, 8.0, -1.0, 1.0);
        let ext = Volume::new(IVec3::new(4, 1, 1), IVec3::new(-3, 40, 0), IVec3::ONE);
        assert_eq!(run(&g, ext), vec![-0.375, -0.25, -0.125, 0.0]);
    }

    #[test]
    fn a_descending_clamped_gradient_keeps_both_coordinates() {
        let g = gradient(Axis::Y, Tiling::ClampToEdge, 64.0, -64.0, 1.0, -1.0);
        let at = |y| one(&g, 0, y, 0);
        assert_eq!(at(64), 1.0);
        assert_eq!(at(-64), -1.0);
        assert_eq!(at(100), 1.0, "clamped to the sorted maximum");
        assert_eq!(at(-100), -1.0);
        // Collapsing the sorted minimum into from_coordinate would answer 2.0.
        assert_eq!(at(0), 0.0);
    }

    #[test]
    fn repeat_wraps_below_the_origin() {
        let g = gradient(Axis::Z, Tiling::Repeat, 0.0, 4.0, 0.0, 4.0);
        assert_eq!(
            one(&g, 0, 0, -1),
            3.0,
            "floor_mod, not the truncating remainder"
        );
        assert_eq!(one(&g, 0, 0, -4), 0.0);
    }

    #[test]
    fn mirrored_repeat_alternates_across_negative_tiles() {
        let g = gradient(Axis::X, Tiling::MirroredRepeat, 0.0, 4.0, 0.0, 4.0);
        let at = |x| one(&g, x, 0, 0);
        assert_eq!(at(0), 0.0);
        assert_eq!(at(3), 3.0);
        assert_eq!(at(5), 3.0, "tile 1 mirrors");
        assert_eq!(at(-1), 1.0, "tile -1 is odd");
        assert_eq!(at(-4), 4.0);
        assert_eq!(at(-5), 3.0);
    }

    #[test]
    fn a_one_sample_extent_agrees_with_the_head_of_a_full_one() {
        let g = gradient(Axis::Y, Tiling::MirroredRepeat, -9.0, 6.0, 0.5, 2.0);
        let step = IVec3::new(1, 4, 1);
        let min = IVec3::new(3, -21, -7);
        let full = run(&g, Volume::new(IVec3::new(1, 6, 1), min, step));
        let head = run(&g, Volume::new(IVec3::ONE, min, step));
        assert_eq!(head[0], full[0]);
    }

    #[test]
    fn the_codec_default_is_clamp_to_edge() {
        assert_eq!(Tiling::default(), Tiling::ClampToEdge);
    }
}
