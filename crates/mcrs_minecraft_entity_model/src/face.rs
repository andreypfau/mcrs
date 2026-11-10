use core::fmt;
use core::str::FromStr;

use glam::Vec3;

/// A face of a cube, named in internal space. The discriminant is the face index of the GPU
/// records and the bit of a face mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Face {
    /// +X, the right side of the entity.
    East = 0,
    /// −Z, the front of the entity.
    North = 1,
    /// −X.
    West = 2,
    /// +Z.
    South = 3,
    /// +Y.
    Up = 4,
    /// −Y.
    Down = 5,
}

/// The two triangles of a face as indices into its corners `c0..c3`. Each triangle turns
/// counter-clockwise when the viewer looks at the outer side of the face.
pub const FACE_TRIANGLES: [[usize; 3]; 2] = [[0, 2, 1], [1, 2, 3]];

impl Face {
    pub const ALL: [Face; 6] = [
        Face::East,
        Face::North,
        Face::West,
        Face::South,
        Face::Up,
        Face::Down,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Face::East => "east",
            Face::North => "north",
            Face::West => "west",
            Face::South => "south",
            Face::Up => "up",
            Face::Down => "down",
        }
    }

    #[must_use]
    pub fn normal(self) -> Vec3 {
        match self {
            Face::East => Vec3::X,
            Face::North => Vec3::NEG_Z,
            Face::West => Vec3::NEG_X,
            Face::South => Vec3::Z,
            Face::Up => Vec3::Y,
            Face::Down => Vec3::NEG_Y,
        }
    }

    #[must_use]
    pub fn mask_bit(self) -> u8 {
        1 << self as u8
    }

    /// The corners `c0`, `c1`, `c2` and `c3` of this face of the box from `lo` to `hi`. `c0`
    /// takes the texel `(u1, v1)`, `c1` `(u2, v1)`, `c2` `(u1, v2)` and `c3` `(u2, v2)`.
    #[must_use]
    pub fn corners(self, lo: Vec3, hi: Vec3) -> [Vec3; 4] {
        let at = Vec3::new;
        match self {
            Face::East => [
                at(hi.x, hi.y, hi.z),
                at(hi.x, hi.y, lo.z),
                at(hi.x, lo.y, hi.z),
                at(hi.x, lo.y, lo.z),
            ],
            Face::West => [
                at(lo.x, hi.y, lo.z),
                at(lo.x, hi.y, hi.z),
                at(lo.x, lo.y, lo.z),
                at(lo.x, lo.y, hi.z),
            ],
            Face::Up => [
                at(lo.x, hi.y, lo.z),
                at(hi.x, hi.y, lo.z),
                at(lo.x, hi.y, hi.z),
                at(hi.x, hi.y, hi.z),
            ],
            Face::Down => [
                at(lo.x, lo.y, hi.z),
                at(hi.x, lo.y, hi.z),
                at(lo.x, lo.y, lo.z),
                at(hi.x, lo.y, lo.z),
            ],
            Face::South => [
                at(lo.x, hi.y, hi.z),
                at(hi.x, hi.y, hi.z),
                at(lo.x, lo.y, hi.z),
                at(hi.x, lo.y, hi.z),
            ],
            Face::North => [
                at(hi.x, hi.y, lo.z),
                at(lo.x, hi.y, lo.z),
                at(hi.x, lo.y, lo.z),
                at(lo.x, lo.y, lo.z),
            ],
        }
    }
}

/// The text names no face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownFace;

impl fmt::Display for UnknownFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("not a face name")
    }
}

impl core::error::Error for UnknownFace {}

impl FromStr for Face {
    type Err = UnknownFace;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Face::ALL
            .into_iter()
            .find(|face| face.name() == name)
            .ok_or(UnknownFace)
    }
}
