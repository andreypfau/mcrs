//! Discrete voxel-shape internal representation: a packed bitset over a
//! sub-block grid. v1 ships only the type scaffold needed for
//! `ShapeRepr::Discrete` to compile and for downstream callers to construct
//! empty discrete shapes with a given bounding box. The full bit-merge
//! face-occludes algorithm and slab/stair/wall constructor helpers land
//! alongside the conditional-shape slow path.

use super::Aabb;

/// Packed-bit representation of a sub-block-resolution voxel mask.
///
/// `resolution` is the per-axis cell count and must be a power of two in
/// `{1, 2, 4, 8}` to match the vanilla `findBits` resolution computation.
/// `bits` is a flat `[u64]` packed YZX-major (the same ordering NibbleArray
/// uses): linear index `y * (rx * rz) + z * rx + x` maps to bit
/// `linear & 63` of word `linear >> 6`.
#[derive(Debug)]
pub struct DiscreteShape {
    pub bounds: Aabb,
    pub resolution: (u8, u8, u8),
    pub bits: Box<[u64]>,
}

impl DiscreteShape {
    /// Construct an all-zero (no filled cells) discrete shape with the given
    /// bounding box. Resolution defaults to `(1, 1, 1)` — a single-cell mask
    /// — which is the minimum legal value and keeps the bit storage to one
    /// word. Helpers that build slab / stair / wall shapes will set the
    /// resolution and bit pattern explicitly.
    pub fn empty_with_bounds(bounds: Aabb) -> Self {
        let resolution = (1u8, 1u8, 1u8);
        let cell_count = resolution.0 as usize
            * resolution.1 as usize
            * resolution.2 as usize;
        let word_count = cell_count.div_ceil(64).max(1);
        Self {
            bounds,
            resolution,
            bits: vec![0u64; word_count].into_boxed_slice(),
        }
    }

    #[inline]
    pub fn bounds(&self) -> &Aabb {
        &self.bounds
    }

    /// Return the number of filled cells in the mask.
    ///
    /// v1 callers do not exercise this path; the full face-occludes merge
    /// algorithm consumes it once the conditional-shape slow path lands.
    #[doc(hidden)]
    pub fn _unimpl_filled_cells(&self) -> u32 {
        unimplemented!("discrete-shape filled-cell count lands with the conditional-shape slow path")
    }

    /// Project this shape onto the named axis-aligned face and merge with
    /// `other` projected on the opposite face. Returns true when the merged
    /// 2-D mask covers the full unit face.
    ///
    /// v1 callers do not exercise this path; the BFS fast path skips the
    /// discrete merge whenever the conditionally-opaque flag is false.
    #[doc(hidden)]
    pub fn _unimpl_face_occludes_merge(&self, _other: &DiscreteShape) -> bool {
        unimplemented!("discrete-shape face-occludes merge lands with the conditional-shape slow path")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::Vec3;

    fn unit_aabb() -> Aabb {
        Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        }
    }

    #[test]
    fn empty_with_bounds_stores_supplied_aabb() {
        let s = DiscreteShape::empty_with_bounds(unit_aabb());
        assert_eq!(s.bounds().min, Vec3::ZERO);
        assert_eq!(s.bounds().max, Vec3::ONE);
    }

    #[test]
    fn empty_with_bounds_allocates_nonzero_bit_storage() {
        let s = DiscreteShape::empty_with_bounds(unit_aabb());
        assert!(!s.bits.is_empty(), "bit storage must be at least one word");
        assert!(s.bits.iter().all(|w| *w == 0), "empty shape has zero bits set");
    }

    #[test]
    fn empty_with_bounds_uses_minimum_resolution() {
        let s = DiscreteShape::empty_with_bounds(unit_aabb());
        assert_eq!(s.resolution, (1, 1, 1));
    }
}
