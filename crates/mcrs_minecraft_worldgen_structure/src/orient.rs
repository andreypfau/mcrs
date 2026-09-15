use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, BoundingBox, Direction, Mirror, Rotation};
use mcrs_minecraft_random::Random;

use crate::piece::Piece;

/// A grid piece's facing, numbered by `Direction.get2DDataValue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Orientation {
    South = 0,
    West = 1,
    North = 2,
    East = 3,
}

impl Orientation {
    pub const ALL: [Orientation; 4] = [
        Orientation::South,
        Orientation::West,
        Orientation::North,
        Orientation::East,
    ];

    /// `Direction.Plane.HORIZONTAL`, in the array order `getRandomDirection` indexes.
    pub const HORIZONTAL: [Orientation; 4] = [
        Orientation::North,
        Orientation::East,
        Orientation::South,
        Orientation::West,
    ];

    /// `StructurePiece.getRandomHorizontalDirection`.
    pub fn random<R: Random>(rng: &mut R) -> Self {
        Self::HORIZONTAL[rng.next_i32_bound(4) as usize]
    }

    pub fn from_data_2d(value: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(value).ok()?).copied()
    }

    pub const fn data_2d(self) -> i32 {
        self as i32
    }

    pub const fn direction(self) -> Direction {
        match self {
            Orientation::South => Direction::South,
            Orientation::West => Direction::West,
            Orientation::North => Direction::North,
            Orientation::East => Direction::East,
        }
    }

    /// `StructurePiece.setOrientation`: how a block state is transformed
    /// before it is placed.
    pub const fn mirror_rotation(self) -> (Mirror, Rotation) {
        match self {
            Orientation::South => (Mirror::LeftRight, Rotation::None),
            Orientation::West => (Mirror::LeftRight, Rotation::Clockwise90),
            Orientation::East => (Mirror::None, Rotation::Clockwise90),
            Orientation::North => (Mirror::None, Rotation::None),
        }
    }
}

/// `StructurePiece.makeBoundingBox`: the box of a `width × height × depth`
/// piece whose minimum corner is `origin`, its width along z when it faces
/// east or west.
pub fn orient_box(
    orientation: Orientation,
    origin: IVec3,
    width: i32,
    height: i32,
    depth: i32,
) -> BoundingBox {
    let (span_x, span_z) = match orientation {
        Orientation::North | Orientation::South => (width, depth),
        Orientation::West | Orientation::East => (depth, width),
    };
    BoundingBox {
        min: origin.into(),
        max: (origin + IVec3::new(span_x - 1, height - 1, span_z - 1)).into(),
    }
}

/// `BoundingBox.orientBox`: the box of a `width × height × depth` piece whose
/// foot stands at `foot` and extends away along `orientation`, shifted by the
/// piece-local `offset` before it is turned.
pub fn orient_box_at(
    orientation: Orientation,
    foot: IVec3,
    offset: IVec3,
    width: i32,
    height: i32,
    depth: i32,
) -> BoundingBox {
    let IVec3 { x, y, z } = foot;
    let IVec3 {
        x: dx,
        y: dy,
        z: dz,
    } = offset;
    let (min_x, min_z, max_x, max_z) = match orientation {
        Orientation::South => (x + dx, z + dz, x + width - 1 + dx, z + depth - 1 + dz),
        Orientation::North => (x + dx, z - depth + 1 + dz, x + width - 1 + dx, z + dz),
        Orientation::West => (x - depth + 1 + dz, z + dx, x + dz, z + width - 1 + dx),
        Orientation::East => (x + dz, z + dx, x + depth - 1 + dz, z + width - 1 + dx),
    };
    BoundingBox {
        min: BlockPos::new(min_x, y + dy, min_z),
        max: BlockPos::new(max_x, y + height - 1 + dy, max_z),
    }
}

/// `StructurePiece.getWorldPos`: a piece-local position in the world, or the
/// position itself for a piece with no orientation.
pub fn world_pos(orientation: Option<Orientation>, bounds: BoundingBox, local: IVec3) -> BlockPos {
    let Some(orientation) = orientation else {
        return local.into();
    };
    let (min, max) = (*bounds.min, *bounds.max);
    let IVec3 { x, y, z } = local;
    let (world_x, world_z) = match orientation {
        Orientation::North => (min.x + x, max.z - z),
        Orientation::South => (min.x + x, min.z + z),
        Orientation::West => (max.x - z, min.z + x),
        Orientation::East => (min.x + z, min.z + x),
    };
    BlockPos::new(world_x, y + min.y, world_z)
}

fn union_of(pieces: &[Piece]) -> BoundingBox {
    pieces
        .iter()
        .map(Piece::bounds)
        .reduce(BoundingBox::union)
        .expect("a box needs a piece")
}

/// `StructurePiecesBuilder.offsetPiecesVertically`.
pub fn offset_vertically(pieces: &mut [Piece], dy: i32) {
    for piece in pieces {
        piece.move_by(IVec3::new(0, dy, 0));
    }
}

/// `StructurePiecesBuilder.moveBelowSeaLevel`: the moved distance.
pub fn move_below_sea_level<R: Random>(
    pieces: &mut [Piece],
    sea_level: i32,
    min_y: i32,
    rng: &mut R,
    offset: i32,
) -> i32 {
    let max_y = sea_level - offset;
    let bounds = union_of(pieces);
    let mut top = bounds.y_span() + min_y + 1;
    if top < max_y {
        top += rng.next_i32_bound(max_y - top);
    }
    let dy = top - bounds.max.y;
    offset_vertically(pieces, dy);
    dy
}

/// `StructurePiecesBuilder.moveInsideHeights`.
pub fn move_inside_heights<R: Random>(
    pieces: &mut [Piece],
    rng: &mut R,
    lowest_allowed: i32,
    highest_allowed: i32,
) {
    let bounds = union_of(pieces);
    let span = highest_allowed - lowest_allowed + 1 - bounds.y_span();
    let bottom = if span > 1 {
        lowest_allowed + rng.next_i32_bound(span)
    } else {
        lowest_allowed
    };
    offset_vertically(pieces, bottom - bounds.min.y);
}

/// `StructurePiece.findCollisionPiece`: the first piece whose box meets `probe`.
pub fn find_collision(pieces: &[Piece], probe: BoundingBox) -> Option<&Piece> {
    pieces.iter().find(|piece| piece.bounds().intersects(probe))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_random::legacy::LegacyRandom;

    fn bounds(min: [i32; 3], max: [i32; 3]) -> BoundingBox {
        BoundingBox {
            min: min.into(),
            max: max.into(),
        }
    }

    #[test]
    fn a_random_orientation_reads_the_horizontal_plane_in_face_order() {
        let mut rng = LegacyRandom::new(5);
        let mut replay = rng.clone();
        let picked: Vec<_> = (0..8).map(|_| Orientation::random(&mut rng)).collect();
        let expected: Vec<_> = (0..8)
            .map(|_| Orientation::HORIZONTAL[replay.next_i32_bound(4) as usize])
            .collect();
        assert_eq!(picked, expected);
        assert_eq!(Orientation::from_data_2d(2), Some(Orientation::North));
        assert_eq!(Orientation::East.data_2d(), 3);
        assert_eq!(Orientation::from_data_2d(-1), None);
    }

    #[test]
    fn the_box_swaps_width_and_depth_along_x() {
        let origin = IVec3::new(16, 64, -32);
        assert_eq!(
            orient_box(Orientation::North, origin, 21, 15, 9),
            bounds([16, 64, -32], [36, 78, -24])
        );
        assert_eq!(
            orient_box(Orientation::East, origin, 21, 15, 9),
            bounds([16, 64, -32], [24, 78, -12])
        );
    }

    #[test]
    fn the_foot_box_extends_away_from_its_foot() {
        let foot = IVec3::new(10, 64, 20);
        let offset = IVec3::new(-1, -3, 0);
        let at = |o| orient_box_at(o, foot, offset, 5, 10, 19);
        assert_eq!(at(Orientation::South), bounds([9, 61, 20], [13, 70, 38]));
        assert_eq!(at(Orientation::North), bounds([9, 61, 2], [13, 70, 20]));
        assert_eq!(at(Orientation::West), bounds([-8, 61, 19], [10, 70, 23]));
        assert_eq!(at(Orientation::East), bounds([10, 61, 19], [28, 70, 23]));
    }

    #[test]
    fn world_positions_follow_the_reference_per_facing() {
        let bounds = bounds([10, 60, 20], [30, 74, 40]);
        let local = IVec3::new(2, 3, 5);
        let at = |o| *world_pos(Some(o), bounds, local);
        assert_eq!(at(Orientation::North), IVec3::new(12, 63, 35));
        assert_eq!(at(Orientation::South), IVec3::new(12, 63, 25));
        assert_eq!(at(Orientation::West), IVec3::new(25, 63, 22));
        assert_eq!(at(Orientation::East), IVec3::new(15, 63, 22));
        assert_eq!(*world_pos(None, bounds, local), local);
    }
}
