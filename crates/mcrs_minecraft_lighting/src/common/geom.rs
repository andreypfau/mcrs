//! Face-axis geometry helpers for the chunk lighting system.
//!
//! The two helpers are mutual inverses: `chunk_xyz_to_face_cell` projects a
//! chunk-local offset onto the two non-normal axes of a face, and
//! `face_cell_to_chunk_xyz` expands an on-face `(cell_a, cell_b)` pair back
//! to a chunk-local `(x, y, z)` triple by restoring the normal axis to its
//! boundary value (0 or 15). The axis-dropping convention is:
//! - Y-normal (Down/Up): drop y, return `(cell_a = x, cell_b = z)`
//! - Z-normal (North/South): drop z, return `(cell_a = x, cell_b = y)`
//! - X-normal (West/East): drop x, return `(cell_a = y, cell_b = z)`

use mcrs_core::voxel_shape::Direction;

/// Extract the two on-face coordinates from a `(off_x, off_y, off_z)` triple
/// for the destination face named by `d`. Returns `(cell_x, cell_z)` matching
/// the `CrossChunkWavefront::new(face, cell_x, cell_z, level)` constructor. The face
/// normal axis is dropped; the remaining two axes are returned in
/// (first-non-normal, second-non-normal) order:
///
/// - Up/Down (Y-normal): `(off_x, off_z)`
/// - North/South (Z-normal): `(off_x, off_y)`
/// - East/West (X-normal): `(off_y, off_z)`
#[inline]
pub(crate) fn chunk_xyz_to_face_cell(d: Direction, off_x: i8, off_y: i8, off_z: i8) -> (u8, u8) {
    match d {
        Direction::Down | Direction::Up => ((off_x & 0xF) as u8, (off_z & 0xF) as u8),
        Direction::North | Direction::South => ((off_x & 0xF) as u8, (off_y & 0xF) as u8),
        Direction::West | Direction::East => ((off_y & 0xF) as u8, (off_z & 0xF) as u8),
    }
}

/// Inverse of `chunk_xyz_to_face_cell`: given an inbound wavefront's
/// destination-frame face plus its on-face `(cell_a, cell_b)` packing,
/// return the destination-chunk-local `(x, y, z)` cell coordinates.
///
/// Y-normal faces drop y, X-normal faces drop x, Z-normal faces drop z.
/// For `Up` the implicit y is 15; for `Down` the implicit y is 0; for
/// `East` x is 15; for `West` x is 0; for `South` z is 15; for `North`
/// z is 0. The two non-normal axes pack the on-face cell coordinates in
/// the same order as `chunk_xyz_to_face_cell` — `(cell_a, cell_b)` where
/// `cell_a` is the first non-normal axis and `cell_b` is the second.
#[inline]
pub(crate) fn face_cell_to_chunk_xyz(face: Direction, cell_a: u8, cell_b: u8) -> (u8, u8, u8) {
    match face {
        Direction::Down => (cell_a, 0, cell_b),
        Direction::Up => (cell_a, 15, cell_b),
        Direction::North => (cell_a, cell_b, 0),
        Direction::South => (cell_a, cell_b, 15),
        Direction::West => (0, cell_a, cell_b),
        Direction::East => (15, cell_a, cell_b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_cell_to_chunk_xyz_covers_six_faces() {
        assert_eq!(face_cell_to_chunk_xyz(Direction::Down, 5, 9), (5, 0, 9));
        assert_eq!(face_cell_to_chunk_xyz(Direction::Up, 5, 9), (5, 15, 9));
        assert_eq!(face_cell_to_chunk_xyz(Direction::North, 5, 9), (5, 9, 0));
        assert_eq!(face_cell_to_chunk_xyz(Direction::South, 5, 9), (5, 9, 15));
        assert_eq!(face_cell_to_chunk_xyz(Direction::West, 5, 9), (0, 5, 9));
        assert_eq!(face_cell_to_chunk_xyz(Direction::East, 5, 9), (15, 5, 9));
    }

    #[test]
    fn chunk_xyz_to_face_cell_round_trip() {
        let faces = [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ];
        for face in faces {
            for cell_a in [0u8, 7, 15] {
                for cell_b in [0u8, 7, 15] {
                    let (x, y, z) = face_cell_to_chunk_xyz(face, cell_a, cell_b);
                    let (a2, b2) = chunk_xyz_to_face_cell(face, x as i8, y as i8, z as i8);
                    assert_eq!(
                        (a2, b2),
                        (cell_a, cell_b),
                        "round-trip failed for {face:?} ({cell_a},{cell_b}) -> ({x},{y},{z}) -> ({a2},{b2})"
                    );
                }
            }
        }
    }
}
