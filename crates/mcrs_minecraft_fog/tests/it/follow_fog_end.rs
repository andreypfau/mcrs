use mcrs_minecraft_fog::follow_fog_end;

#[test]
fn fog_end_moves_toward_a_farther_target_and_drops_to_a_nearer_one() {
    let cases = [
        ("first frame of one tick", 0.0, 80.0, 1.0, 8.0),
        ("second frame of one tick", 8.0, 80.0, 1.0, 15.2),
        ("third frame of one tick", 15.2, 80.0, 1.0, 21.68),
        ("ten ticks", 0.0, 80.0, 10.0, 80.0),
        ("twenty ticks", 0.0, 80.0, 20.0, 80.0),
        ("within the snap of the target", 191.995, 192.0, 1.0, 192.0),
        ("nearer target", 192.0, 80.0, 1.0, 80.0),
        ("same target", 80.0, 80.0, 1.0, 80.0),
    ];
    for (name, previous, target, ticks, expected) in cases {
        let end = follow_fog_end(previous, target, ticks);
        assert!(
            (end - expected).abs() <= 1e-4,
            "{name}: got {end}, the specification states {expected}"
        );
    }
}
