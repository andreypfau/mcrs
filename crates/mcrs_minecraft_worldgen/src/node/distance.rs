use crate::jmath::{jmax, mul_add, sqrt};
use crate::volume::Volume;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    Euclidean,
    EuclideanSquared,
    Manhattan,
    Chebyshev,
}

impl DistanceMetric {
    /// The squared sum accumulates in `f32`, as `Mth.lengthSquared` does.
    /// Summing in `f64` and narrowing at the end moves the last bits.
    #[inline]
    fn length_squared(dx: f32, dy: f32, dz: f32) -> f32 {
        mul_add(dz, dz, mul_add(dy, dy, dx * dx))
    }

    #[inline]
    pub fn compute(self, dx: f32, dy: f32, dz: f32) -> f32 {
        match self {
            Self::Euclidean => sqrt(Self::length_squared(dx, dy, dz)),
            Self::EuclideanSquared => Self::length_squared(dx, dy, dz),
            Self::Manhattan => dx.abs() + dy.abs() + dz.abs(),
            Self::Chebyshev => jmax(jmax(dx.abs(), dy.abs()), dz.abs()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DistanceParams {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub metric: DistanceMetric,
}

impl DistanceParams {
    pub fn new(x: i32, y: i32, z: i32, metric: DistanceMetric) -> Self {
        Self { x, y, z, metric }
    }

    pub fn eval(&self, out: &mut [f32], ext: &Volume) {
        let size = ext.size();
        let metric = self.metric;
        let mut i = 0usize;
        for iz in 0..size.z {
            let dz = self.z.wrapping_sub(ext.block_z(iz)) as f32;
            for ix in 0..size.x {
                let dx = self.x.wrapping_sub(ext.block_x(ix)) as f32;
                for iy in 0..size.y {
                    let dy = self.y.wrapping_sub(ext.block_y(iy)) as f32;
                    out[i] = metric.compute(dx, dy, dz);
                    i += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::IVec3;

    fn run(params: &DistanceParams, ext: Volume) -> Vec<f32> {
        let mut out = vec![0.0; ext.len()];
        params.eval(&mut out, &ext);
        out
    }

    fn at(params: &DistanceParams, x: i32, y: i32, z: i32) -> f32 {
        run(params, Volume::point(IVec3::new(x, y, z)))[0]
    }

    /// The fast profile fuses the products into the sum. That is one correctly
    /// rounded result rather than a wider accumulator, but on this offset it
    /// happens to land on the value widening would have given, so the narrow
    /// accumulation is pinned in the strict profile alone.
    #[test]
    fn the_square_sum_rounds_in_f32_not_at_the_end() {
        let p = DistanceParams::new(3, 4, 4097, DistanceMetric::EuclideanSquared);
        let widened = (3i64 * 3 + 4 * 4 + 4097i64 * 4097) as f64 as f32;
        assert_eq!(widened, 16785434.0);
        #[cfg(not(feature = "fast"))]
        assert_eq!(at(&p, 0, 0, 0), 16785432.0, "an f64 accumulation answers {widened}");
        #[cfg(feature = "fast")]
        assert_eq!(at(&p, 0, 0, 0), widened);
    }

    #[test]
    fn metrics_disagree_on_the_same_offset() {
        let value = |m| at(&DistanceParams::new(3, -4, 12, m), 0, 0, 0);
        assert_eq!(value(DistanceMetric::Euclidean), 13.0);
        assert_eq!(value(DistanceMetric::EuclideanSquared), 169.0);
        assert_eq!(value(DistanceMetric::Manhattan), 19.0);
        assert_eq!(value(DistanceMetric::Chebyshev), 12.0);
    }

    #[test]
    fn the_delta_wraps_like_java_int_subtraction() {
        let p = DistanceParams::new(i32::MIN, 0, 0, DistanceMetric::Chebyshev);
        assert_eq!(at(&p, 1, 0, 0), i32::MAX as f32);
    }

    #[test]
    fn the_extent_is_walked_y_fastest_then_x_then_z() {
        let p = DistanceParams::new(0, 0, 0, DistanceMetric::Manhattan);
        let ext = Volume::new(IVec3::new(2, 2, 2), IVec3::new(1, -1, 3), IVec3::ONE);
        let got = run(&p, ext);
        let mut want = Vec::new();
        for z in 3..5 {
            for x in 1..3 {
                for y in -1i32..1 {
                    want.push((x + y.abs() + z) as f32);
                }
            }
        }
        assert_eq!(got, want);
    }

    #[test]
    fn a_run_walks_the_column_with_the_step() {
        let p = DistanceParams::new(0, 0, 0, DistanceMetric::Manhattan);
        let ext = Volume::new(
            IVec3::new(1, 4, 1),
            IVec3::new(2, -8, -3),
            IVec3::new(1, 4, 1),
        );
        assert_eq!(run(&p, ext), vec![13.0, 9.0, 5.0, 9.0]);
    }
}
