/// The 16-entry Ken Perlin gradient table shared by the Perlin (`ImprovedNoise`) and
/// simplex (`SimplexNoise`) generators.
///
/// `ImprovedNoise` indexes it with `hash & 15` and dots against the full 3D offset.
/// `SimplexNoise` indexes it with `hash % 12` and dots against a 2D offset (z = 0), which
/// selects the first twelve gradients projected onto the XY plane — the classic 12-entry
/// simplex gradient set. Both Beta (`NoiseGenerator2`/`NoiseGeneratorPerlin`) and modern
/// vanilla (`SimplexNoise`/`ImprovedNoise`) use these same vectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gradient {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Gradient {
    #[inline(always)]
    pub const fn dot(self, x: f64, y: f64, z: f64) -> f64 {
        self.x * x + self.y * y + self.z * z
    }
}

const fn g(x: f64, y: f64, z: f64) -> Gradient {
    Gradient { x, y, z }
}

pub const GRADIENTS: [Gradient; 16] = [
    g(1.0, 1.0, 0.0),
    g(-1.0, 1.0, 0.0),
    g(1.0, -1.0, 0.0),
    g(-1.0, -1.0, 0.0),
    g(1.0, 0.0, 1.0),
    g(-1.0, 0.0, 1.0),
    g(1.0, 0.0, -1.0),
    g(-1.0, 0.0, -1.0),
    g(0.0, 1.0, 1.0),
    g(0.0, -1.0, 1.0),
    g(0.0, 1.0, -1.0),
    g(0.0, -1.0, -1.0),
    g(1.0, 1.0, 0.0),
    g(0.0, -1.0, 1.0),
    g(-1.0, 1.0, 0.0),
    g(0.0, -1.0, -1.0),
];
