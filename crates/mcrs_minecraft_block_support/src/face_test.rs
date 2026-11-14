use mcrs_minecraft_block::definition::placement_filter::FaceTest;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::voxel_shape::Aabb;

const EPSILON: f32 = 1.0e-4;

/// A rectangle on the square of a face, in sixteenths: `[min_u, min_v, max_u, max_v]`.
type Rect = [f32; 4];

/// Whether the face `direction` of `shape` passes `test` (SUP-FACE-001).
pub fn passes(shape: &[Aabb], direction: Direction, test: FaceTest) -> bool {
    let area: Vec<Rect> = shape
        .iter()
        .filter_map(|aabb| face_rect(aabb, direction))
        .collect();
    let regions: &[Rect] = match test {
        FaceTest::Full => &[[0.0, 0.0, 16.0, 16.0]],
        FaceTest::Center => &[[7.0, 7.0, 9.0, 9.0]],
        FaceTest::Ring => &[
            [0.0, 0.0, 16.0, 2.0],
            [0.0, 14.0, 16.0, 16.0],
            [0.0, 2.0, 2.0, 14.0],
            [14.0, 2.0, 16.0, 14.0],
        ],
    };
    regions.iter().all(|region| covers(&area, region))
}

fn face_rect(aabb: &Aabb, direction: Direction) -> Option<Rect> {
    let axis = direction.axis() as usize;
    let plane = if direction.is_positive() { 1.0 } else { 0.0 };
    let touches = aabb.min[axis] <= plane + EPSILON && aabb.max[axis] >= plane - EPSILON;
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    touches.then(|| {
        [aabb.min[u], aabb.min[v], aabb.max[u], aabb.max[v]].map(|coordinate| coordinate * 16.0)
    })
}

/// Whether the union of `rects` covers `region`: each cell of the grid that the edges of both
/// cut out of the region has its centre in some rectangle.
fn covers(rects: &[Rect], region: &Rect) -> bool {
    let cuts = |span: fn(&Rect) -> [f32; 2]| {
        let [low, high] = span(region);
        let mut cuts: Vec<f32> = rects
            .iter()
            .flat_map(span)
            .chain([low, high])
            .filter(|cut| (low..=high).contains(cut))
            .collect();
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < EPSILON);
        cuts
    };
    let (us, vs) = (
        cuts(|rect| [rect[0], rect[2]]),
        cuts(|rect| [rect[1], rect[3]]),
    );
    us.windows(2).all(|u| {
        vs.windows(2).all(|v| {
            let (Some(&[u0, u1]), Some(&[v0, v1])) = (u.first_chunk(), v.first_chunk()) else {
                return true;
            };
            let centre = [(u0 + u1) / 2.0, (v0 + v1) / 2.0];
            rects.iter().any(|rect| {
                (rect[0]..=rect[2]).contains(&centre[0]) && (rect[1]..=rect[3]).contains(&centre[1])
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn sixteenths(min: [f32; 3], max: [f32; 3]) -> Aabb {
        Aabb {
            min: Vec3::from(min) / 16.0,
            max: Vec3::from(max) / 16.0,
        }
    }

    #[test]
    fn each_face_test_reads_its_region_of_the_face() {
        let cube = [sixteenths([0.0; 3], [16.0; 3])];
        let bottom_slab = [sixteenths([0.0; 3], [16.0, 8.0, 16.0])];
        let fence_post = [sixteenths([6.0, 0.0, 6.0], [10.0, 24.0, 10.0])];
        let above = [sixteenths([0.0, 18.0, 0.0], [16.0, 24.0, 16.0])];
        let plate_ring = [
            sixteenths([0.0, 0.0, 0.0], [16.0, 16.0, 2.0]),
            sixteenths([0.0, 0.0, 14.0], [16.0, 16.0, 16.0]),
            sixteenths([0.0, 0.0, 2.0], [2.0, 16.0, 14.0]),
            sixteenths([14.0, 0.0, 2.0], [16.0, 16.0, 14.0]),
        ];
        let cases: [(&[Aabb], [bool; 3]); 6] = [
            (&cube, [true, true, true]),
            (&bottom_slab, [false, false, false]),
            (&fence_post, [false, true, false]),
            (&above, [false, false, false]),
            (&plate_ring, [false, false, true]),
            (&[], [false, false, false]),
        ];
        for (shape, expected) in cases {
            let results = [FaceTest::Full, FaceTest::Center, FaceTest::Ring]
                .map(|test| passes(shape, Direction::Up, test));
            assert_eq!(results, expected, "{shape:?}");
        }
        assert!(passes(&bottom_slab, Direction::Down, FaceTest::Full));
        assert!(!passes(&bottom_slab, Direction::East, FaceTest::Full));
    }
}
