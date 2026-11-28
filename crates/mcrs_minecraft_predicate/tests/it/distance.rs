use glam::DVec3;
use mcrs_minecraft_predicate::DistancePredicate;

#[test]
fn the_distance_predicate_tests_each_axis_and_the_squared_sums_pred_dst_001() {
    let origin = DVec3::new(10.0, 64.0, -5.0);
    let cases: [(&str, DVec3, bool); 9] = [
        (r#"{"x": {"max": 3.0}}"#, DVec3::new(3.0, 0.0, 0.0), true),
        (r#"{"x": {"max": 3.0}}"#, DVec3::new(-3.0, 0.0, 0.0), true),
        (r#"{"x": {"max": 2.99}}"#, DVec3::new(3.0, 0.0, 0.0), false),
        (r#"{"y": {"min": 1.0}}"#, DVec3::new(5.0, 0.5, 0.0), false),
        (
            r#"{"horizontal": {"max": 5.0}}"#,
            DVec3::new(3.0, 9.0, 4.0),
            true,
        ),
        (
            r#"{"horizontal": {"max": 5.0}}"#,
            DVec3::new(3.0, 0.0, 4.01),
            false,
        ),
        (
            r#"{"absolute": {"max": 3.0}}"#,
            DVec3::new(1.0, 2.0, 2.0),
            true,
        ),
        (
            r#"{"absolute": {"min": 3.01}}"#,
            DVec3::new(1.0, 2.0, 2.0),
            false,
        ),
        ("{}", DVec3::new(1.0e6, 0.0, 0.0), true),
    ];
    for (json, offset, expected) in cases {
        let predicate: DistancePredicate = serde_json::from_str(json).unwrap();
        let other = origin + offset;
        assert_eq!(
            predicate.matches(origin, other),
            expected,
            "{json} at {offset}"
        );
        assert_eq!(
            predicate.matches(other, origin),
            expected,
            "{json} swapped at {offset}"
        );
    }
}
