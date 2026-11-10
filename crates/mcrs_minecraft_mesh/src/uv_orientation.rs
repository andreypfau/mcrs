use crate::block::CORNER_UV;

/// How a cube face lays its whole sprite on its cell: one of the eight turns and mirrors of the
/// unit square. The terrain shaders apply it to the coordinates inside each cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UvOrientation(u8);

const SWAP: u8 = 1;
const FLIP_U: u8 = 2;
const FLIP_V: u8 = 4;
const TOLERANCE: f32 = 1e-4;

impl UvOrientation {
    pub const IDENTITY: UvOrientation = UvOrientation(0);

    /// The orientation whose corners are `uvs`, the four corners in the order of [`CORNER_UV`],
    /// or `None` when `uvs` do not cover the whole sprite.
    #[must_use]
    pub fn of_corners(uvs: &[[f32; 2]; 4]) -> Option<Self> {
        (0..8).map(UvOrientation).find(|orientation| {
            CORNER_UV.iter().zip(uvs).all(|(&corner, &[u, v])| {
                let [want_u, want_v] = orientation.apply(corner);
                (u - want_u).abs() <= TOLERANCE && (v - want_v).abs() <= TOLERANCE
            })
        })
    }

    /// The sprite coordinates at the coordinates `uv` inside the cell.
    #[must_use]
    pub fn apply(self, [u, v]: [f32; 2]) -> [f32; 2] {
        let [mut u, mut v] = if self.0 & SWAP != 0 { [v, u] } else { [u, v] };
        if self.0 & FLIP_U != 0 {
            u = 1.0 - u;
        }
        if self.0 & FLIP_V != 0 {
            v = 1.0 - v;
        }
        [u, v]
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_of_the_eight_orientations_is_read_back_from_its_corners() {
        let mut seen = Vec::new();
        for bits in 0..8 {
            let orientation = UvOrientation(bits);
            let corners = CORNER_UV.map(|corner| orientation.apply(corner));
            assert!(!seen.contains(&corners), "orientation {bits} repeats one");
            seen.push(corners);
            assert_eq!(UvOrientation::of_corners(&corners), Some(orientation));
        }
    }

    #[test]
    fn corners_that_do_not_cover_the_whole_sprite_have_no_orientation() {
        let half = CORNER_UV.map(|[u, v]| [u * 0.5, v]);
        assert_eq!(UvOrientation::of_corners(&half), None);
    }
}
