use crate::volume::{Axis, Volume};
use bevy_math::IVec3;

pub type Axes = u8;

pub const NO_AXES: Axes = 0;
pub const AXIS_X: Axes = 1;
pub const AXIS_Y: Axes = 2;
pub const AXIS_Z: Axes = 4;
pub const ALL_AXES: Axes = 7;

/// `DensityFunction.axesFrom`.
#[inline]
pub const fn axis_bit(axis: Axis) -> Axes {
    match axis {
        Axis::X => AXIS_X,
        Axis::Y => AXIS_Y,
        Axis::Z => AXIS_Z,
    }
}

/// How many values a node of this stratum holds within one column: `sy` if it
/// varies along Y, otherwise a single scalar broadcast over the column.
#[inline]
pub const fn run_len(axes: Axes, size_y: usize) -> usize {
    if axes & AXIS_Y != 0 { size_y } else { 1 }
}

/// The extent of a stratum over a whole volume, which is what one node's buffer
/// costs.
pub fn extent(axes: Axes, volume: &Volume) -> usize {
    let s = volume.size();
    let x = if axes & AXIS_X != 0 { s.x as usize } else { 1 };
    let y = if axes & AXIS_Y != 0 { s.y as usize } else { 1 };
    let z = if axes & AXIS_Z != 0 { s.z as usize } else { 1 };
    x * y * z
}

/// `volume` with every axis the stratum drops collapsed to one sample. The
/// result indexes a stratum buffer exactly as [`Volume::index_unchecked`] does,
/// so an `ALL_AXES` buffer is already the fill's output.
///
/// A dropped axis still needs some coordinate. A dropped Y reads the volume's
/// own first row; a dropped X or Z reads block zero, and that is pinned rather
/// than arbitrary: a noise sheds X and Z by having `xz_scale` be zero, and
/// `block * 0.0` still carries the sign of the block.
pub fn stratum(axes: Axes, volume: &Volume) -> Volume {
    let (size, min) = (volume.size(), volume.min_block());
    let keep = |bit: Axes, n: i32| if axes & bit != 0 { n } else { 1 };
    Volume::new(
        IVec3::new(
            keep(AXIS_X, size.x),
            keep(AXIS_Y, size.y),
            keep(AXIS_Z, size.z),
        ),
        IVec3::new(
            if axes & AXIS_X != 0 { min.x } else { 0 },
            min.y,
            if axes & AXIS_Z != 0 { min.z } else { 0 },
        ),
        volume.step_block(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell() -> Volume {
        Volume::dense(IVec3::new(4, 8, 4), IVec3::ZERO)
    }

    #[test]
    fn strata_shrink_the_work_by_the_axes_a_node_ignores() {
        let v = cell();
        assert_eq!(extent(ALL_AXES, &v), 128);
        assert_eq!(extent(AXIS_X | AXIS_Z, &v), 16);
        assert_eq!(extent(AXIS_Y, &v), 8);
        assert_eq!(extent(NO_AXES, &v), 1);
    }

    #[test]
    fn a_full_stratum_indexes_exactly_like_the_volume() {
        let v = Volume::new(
            IVec3::new(4, 8, 4),
            IVec3::new(3, -64, -7),
            IVec3::new(2, 4, 2),
        );
        let s = stratum(ALL_AXES, &v);
        assert_eq!(s, v);
        for iz in 0..4 {
            for ix in 0..4 {
                for iy in 0..8 {
                    assert_eq!(s.index_unchecked(ix, iy, iz), v.index_unchecked(ix, iy, iz));
                }
            }
        }
    }

    #[test]
    fn a_dropped_axis_collapses_to_one_sample_at_a_fixed_coordinate() {
        let v = Volume::new(
            IVec3::new(4, 8, 4),
            IVec3::new(3, -64, -7),
            IVec3::new(2, 4, 2),
        );
        let s = stratum(AXIS_Y, &v);
        assert_eq!(s.size(), IVec3::new(1, 8, 1));
        assert_eq!(s.block_x(0), 0);
        assert_eq!(s.block_z(0), 0);
        assert_eq!(s.block_y(0), -64);

        let xz = stratum(AXIS_X | AXIS_Z, &v);
        assert_eq!(xz.size(), IVec3::new(4, 1, 4));
        assert_eq!(
            xz.block_y(0),
            -64,
            "a dropped Y reads the volume's first row"
        );
        assert_eq!(xz.block_x(1), 5);
    }

    #[test]
    fn a_run_is_the_column_height_or_a_scalar() {
        assert_eq!(run_len(ALL_AXES, 8), 8);
        assert_eq!(run_len(AXIS_Y, 8), 8);
        assert_eq!(run_len(AXIS_X | AXIS_Z, 8), 1);
        assert_eq!(run_len(NO_AXES, 8), 1);
    }
}
