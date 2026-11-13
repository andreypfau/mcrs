use glam::DVec3;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_item_entity::default_form;
use mcrs_minecraft_item_entity::hand_form;

const FEET: DVec3 = DVec3::new(0.5, -60.0, 0.5);

fn streams() -> impl Iterator<Item = KeyedStream> {
    (0..2000_u128).map(|key| KeyedStream::new(&EntityKey(key), 7))
}

fn bounds(starts: &[Start], value: impl Fn(&Start) -> f64) -> (f64, f64) {
    starts
        .iter()
        .map(value)
        .fold((f64::MAX, f64::MIN), |(min, max), value| {
            (min.min(value), max.max(value))
        })
}

#[test]
fn ien_spn_002_the_look_velocity_is_0_3_along_the_look_plus_a_lift_and_a_spread() {
    let directions = [(0.0, 0.0), (90.0, 0.0), (0.0, -90.0), (45.0, 30.0)];
    for (yaw, pitch) in directions {
        let starts: Vec<Start> = streams()
            .map(|stream| hand_form(FEET, 1.62, HandVelocity::Look { yaw, pitch }, &stream))
            .collect();
        let (yaw_rad, pitch_rad) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
        let look = DVec3::new(
            -yaw_rad.sin() * pitch_rad.cos(),
            -pitch_rad.sin(),
            yaw_rad.cos() * pitch_rad.cos(),
        ) * 0.3;
        let (low, high) = bounds(&starts, |start| start.velocity.y - look.y);
        assert!(
            (0.0..0.03).contains(&low) && (0.17..0.2).contains(&high),
            "({yaw}, {pitch}): the lift spans 0 to 0.2, {low} to {high}"
        );
        let (_, spread) = bounds(&starts, |start| {
            (start.velocity - look).with_y(0.0).length()
        });
        assert!(
            (0.019..0.0201).contains(&spread),
            "({yaw}, {pitch}): the spread reaches 0.02, {spread}"
        );
        let lifts: f64 = starts.iter().map(|start| start.velocity.y - look.y).sum();
        let mean = lifts / starts.len() as f64;
        assert!(
            (mean - 0.1).abs() < 0.005,
            "the lift has the mean 0.1: {mean}"
        );
    }
}

#[test]
fn ien_spn_002_the_hand_form_starts_0_3_below_the_eyes_and_scatters_at_0_2_up() {
    let starts: Vec<Start> = streams()
        .map(|stream| hand_form(FEET, 1.62, HandVelocity::Scatter, &stream))
        .collect();
    let eyes = FEET.y + f64::from(1.62_f32) - f64::from(0.3_f32);
    assert!(
        starts
            .iter()
            .all(|start| start.position == FEET.with_y(eyes)
                && start.velocity.y == f64::from(0.2_f32)),
        "position and vertical velocity are fixed"
    );
    let (_, reach) = bounds(&starts, |start| start.velocity.with_y(0.0).length());
    assert!(
        (0.49..0.5).contains(&reach),
        "the scatter reaches 0.5: {reach}"
    );
}

#[test]
fn ien_spn_008_the_default_form_starts_at_the_point_at_0_2_up_within_0_1() {
    let starts: Vec<Start> = streams()
        .map(|stream| default_form(FEET, &stream))
        .collect();
    for axis in [
        |start: &Start| start.velocity.x,
        |start: &Start| start.velocity.z,
    ] {
        let (low, high) = bounds(&starts, axis);
        assert!(
            (-0.1..-0.099).contains(&low) && (0.099..0.1).contains(&high),
            "{low} to {high}"
        );
    }
    assert!(
        starts
            .iter()
            .all(|start| start.position == FEET && start.velocity.y == 0.2),
        "position and vertical velocity are fixed"
    );
}
