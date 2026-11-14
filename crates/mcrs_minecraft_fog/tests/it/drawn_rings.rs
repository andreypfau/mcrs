use std::ops::RangeInclusive;

use mcrs_minecraft_fog::DrawnRings;

const RENDER_DISTANCE: u8 = 12;

enum Step {
    InsertRings(RangeInclusive<u32>),
    Insert(i32, i32),
    Remove(i32, i32),
}

fn passing_columns_of_rings(rings: RangeInclusive<u32>) -> impl Iterator<Item = (i32, i32)> {
    let reach = i32::from(RENDER_DISTANCE) + 2;
    (-reach..=reach)
        .flat_map(move |dx| (-reach..=reach).map(move |dz| (dx, dz)))
        .filter(move |&(dx, dz)| {
            let gap = |offset: i32| f64::from((offset.abs() - 1).max(0));
            let squared = gap(dx).powi(2) + gap(dz).powi(2);
            let passes = squared < f64::from(RENDER_DISTANCE).powi(2);
            passes && rings.contains(&(squared.sqrt().floor() as u32))
        })
}

#[test]
fn fog_end_target_is_the_first_ring_with_a_missing_column() {
    let steps = [
        ("rings 0 to 4 drawn", Step::InsertRings(0..=4), 80.0),
        (
            "every passing column drawn",
            Step::InsertRings(5..=11),
            192.0,
        ),
        ("a column of the ring 5 removed", Step::Remove(6, 0), 80.0),
        ("that column drawn again", Step::Insert(6, 0), 192.0),
        ("a failing column removed", Step::Remove(13, 0), 192.0),
        ("a failing corner removed", Step::Remove(-10, 10), 192.0),
        ("a failing column inserted", Step::Insert(0, -14), 192.0),
        (
            "a column of the ring 5 removed again",
            Step::Remove(0, 6),
            80.0,
        ),
        (
            "a failing column inserted with a ring missing",
            Step::Insert(13, 1),
            80.0,
        ),
    ];
    let mut rings = DrawnRings::new(RENDER_DISTANCE);
    assert_eq!(rings.fog_end_target(), 0.0, "nothing drawn");
    for (name, step, target) in steps {
        match step {
            Step::InsertRings(ring_range) => {
                for (dx, dz) in passing_columns_of_rings(ring_range) {
                    rings.insert(dx, dz);
                }
            }
            Step::Insert(dx, dz) => rings.insert(dx, dz),
            Step::Remove(dx, dz) => rings.remove(dx, dz),
        }
        assert_eq!(rings.fog_end_target(), target, "{name}");
    }
}
