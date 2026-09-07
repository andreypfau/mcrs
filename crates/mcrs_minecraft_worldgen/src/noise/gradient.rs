/// The 16-entry Ken Perlin gradient table shared by the Perlin (`ImprovedNoise`) and
/// simplex (`SimplexNoise`) generators, flattened for the hot paths.
///
/// `ImprovedNoise` indexes it with `hash & 15` and dots against the full 3D offset.
/// `SimplexNoise` indexes it with `hash % 12` and dots against a 2D offset (z = 0), which
/// selects the first twelve gradients projected onto the XY plane — the classic 12-entry
/// simplex gradient set. Both Beta (`NoiseGenerator2`/`NoiseGeneratorPerlin`) and modern
/// vanilla (`SimplexNoise`/`ImprovedNoise`) use these same vectors.
///
/// Both generators dot the same 16 vectors against an offset; only the width of
/// the arithmetic differs, because each path is pinned to a different oracle —
/// modern vanilla computes noise in float, Beta 1.7.3 in double. Keeping the
/// table and the index arithmetic here means an optimization to either lands on
/// both paths, and the width stays a type parameter rather than a fork.
pub trait NoiseFloat: num_traits::Float {
    /// 16 gradients × {x, y, z, pad}, so a lookup is a shift instead of a multiply.
    const GRAD_FLAT: [Self; 64];

    /// Narrow a coordinate to the value width. Positions stay `f64` on both
    /// paths — as they do in vanilla — and only the sampled value follows `Self`.
    fn from_f64(value: f64) -> Self;

    /// Dot the gradient at `index` — already `(hash & 15) << 2` — against the offset.
    ///
    /// Masking with 60 keeps the three reads provably inside the table, which is
    /// what lets the bounds checks go without `unsafe`.
    #[inline(always)]
    fn grad_dot_at(index: usize, x: Self, y: Self, z: Self) -> Self {
        let i = index & 60;
        Self::GRAD_FLAT[i] * x + Self::GRAD_FLAT[i | 1] * y + Self::GRAD_FLAT[i | 2] * z
    }

    /// Dot without the y term — vanilla's `dotXz`. The 2D fills and the modern
    /// path's column split both need the xz plane on its own.
    #[inline(always)]
    fn grad_dot_xz_at(index: usize, x: Self, z: Self) -> Self {
        let i = index & 60;
        Self::GRAD_FLAT[i] * x + Self::GRAD_FLAT[i | 2] * z
    }

    /// The gradient's y component, which the modern path carries as the slope of
    /// a column rather than folding it into the dot.
    #[inline(always)]
    fn grad_y_at(index: usize) -> Self {
        Self::GRAD_FLAT[(index & 60) | 1]
    }

    #[inline(always)]
    fn grad_dot(hash: usize, x: Self, y: Self, z: Self) -> Self {
        Self::grad_dot_at((hash & 15) << 2, x, y, z)
    }

    #[inline(always)]
    fn grad_dot_xz(hash: usize, x: Self, z: Self) -> Self {
        Self::grad_dot_xz_at((hash & 15) << 2, x, z)
    }
}

macro_rules! flat_gradients {
    () => {
        [
            1.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0,
            1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, -1.0, 0.0, -1.0, 0.0,
            0.0, 1.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0,
            1.0, 1.0, 0.0, 0.0, 0.0, -1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0,
        ]
    };
}

impl NoiseFloat for f64 {
    const GRAD_FLAT: [f64; 64] = flat_gradients!();

    #[inline(always)]
    fn from_f64(value: f64) -> f64 {
        value
    }
}

impl NoiseFloat for f32 {
    const GRAD_FLAT: [f32; 64] = flat_gradients!();

    #[inline(always)]
    fn from_f64(value: f64) -> f32 {
        value as f32
    }

    /// The f32 path's parity was captured against vanilla with these three
    /// products fused, so the rounding of the fused form is part of the contract.
    #[inline(always)]
    fn grad_dot_at(index: usize, x: f32, y: f32, z: f32) -> f32 {
        let i = index & 60;
        Self::GRAD_FLAT[i | 2].mul_add(
            z,
            Self::GRAD_FLAT[i | 1].mul_add(y, Self::GRAD_FLAT[i] * x),
        )
    }

    #[inline(always)]
    fn grad_dot_xz_at(index: usize, x: f32, z: f32) -> f32 {
        let i = index & 60;
        Self::GRAD_FLAT[i | 2].mul_add(z, Self::GRAD_FLAT[i] * x)
    }
}
