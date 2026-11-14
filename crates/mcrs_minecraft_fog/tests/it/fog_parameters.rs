use mcrs_minecraft_fog::{FogInputs, FogParameters, FogRamp};

const COLOR_TOLERANCE: f32 = 2.5e-5;
const DISTANCE_TOLERANCE: f32 = 1e-4;
const TWELVE_CHUNKS: FogRamp = FogRamp {
    start: 172.8,
    end: 192.0,
};

struct ParityCase {
    name: &'static str,
    inputs: FogInputs,
    color: [f32; 3],
    environmental: Option<FogRamp>,
    render_distance: FogRamp,
    sky_end: f32,
    cloud_end: f32,
}

struct SunriseCase {
    name: &'static str,
    sun_angle: f32,
    view_east: f32,
    render_distance: u8,
    gray: f32,
}

fn parity_scene(render_distance: u8) -> FogInputs {
    FogInputs {
        fog_color: [0.752941, 0.847059, 1.0],
        sky_color: [0.470588, 0.654902, 1.0],
        sunrise_color: [0.0; 3],
        sunrise_alpha: 0.0,
        sun_angle: 0.0,
        fog_start_distance: 0.0,
        fog_end_distance: 1024.0,
        sky_fog_end_distance: 512.0,
        cloud_fog_end_distance: 2048.0,
        render_distance,
        cloud_distance: 128,
        view_east: 0.0,
        rain_level: 0.0,
        thunder_level: 0.0,
    }
}

fn assert_near(case: &str, quantity: &str, actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{case}, {quantity}: got {actual}, the specification states {expected}"
    );
}

fn assert_ramp(case: &str, quantity: &str, actual: FogRamp, expected: FogRamp) {
    assert_near(
        case,
        quantity,
        actual.start,
        expected.start,
        DISTANCE_TOLERANCE,
    );
    assert_near(case, quantity, actual.end, expected.end, DISTANCE_TOLERANCE);
}

fn assert_parity(case: &ParityCase) {
    let fog = FogParameters::from_inputs(&case.inputs);
    let channels = fog.color.into_iter().zip(case.color);
    for (actual, expected) in channels {
        assert_near(case.name, "C", actual, expected, COLOR_TOLERANCE);
    }
    if let Some(environmental) = case.environmental {
        assert_ramp(case.name, "es, ee", fog.environmental, environmental);
    }
    assert_ramp(
        case.name,
        "rs, re",
        fog.render_distance,
        case.render_distance,
    );
    assert_near(
        case.name,
        "se",
        fog.sky_end,
        case.sky_end,
        DISTANCE_TOLERANCE,
    );
    assert_near(
        case.name,
        "ce",
        fog.cloud_end,
        case.cloud_end,
        DISTANCE_TOLERANCE,
    );
}

#[test]
fn parity_tests_give_the_fog_parameters() {
    let cases = [
        ParityCase {
            name: "CFOG-TEST-001",
            inputs: parity_scene(12),
            color: [0.711644, 0.818954, 1.0],
            environmental: Some(FogRamp::NONE),
            render_distance: TWELVE_CHUNKS,
            sky_end: 192.0,
            cloud_end: 2048.0,
        },
        ParityCase {
            name: "CFOG-TEST-002",
            inputs: parity_scene(5),
            color: [176.05 / 255.0, 205.14 / 255.0, 1.0],
            environmental: Some(FogRamp::NONE),
            render_distance: FogRamp {
                start: 72.0,
                end: 80.0,
            },
            sky_end: 80.0,
            cloud_end: 2048.0,
        },
        ParityCase {
            name: "CFOG-TEST-003",
            inputs: FogInputs {
                fog_color: [0.044291, 0.049827, 0.086275],
                sky_color: [0.0; 3],
                ..parity_scene(12)
            },
            color: [0.037813, 0.042539, 0.073656],
            environmental: Some(FogRamp::NONE),
            render_distance: TWELVE_CHUNKS,
            sky_end: 192.0,
            cloud_end: 2048.0,
        },
        ParityCase {
            name: "CFOG-TEST-004",
            inputs: FogInputs {
                fog_color: [0.374994, 0.421869, 0.6],
                sky_color: [0.404553, 0.450631, 0.536906],
                rain_level: 1.0,
                ..parity_scene(12)
            },
            color: [0.349732, 0.393120, 0.559360],
            // The stated `es` and `ee` take the rain fog factor 1, which the crate does not take.
            environmental: None,
            render_distance: TWELVE_CHUNKS,
            sky_end: 192.0,
            cloud_end: 2048.0,
        },
        ParityCase {
            name: "dense fog",
            inputs: FogInputs {
                fog_start_distance: 10.0,
                fog_end_distance: 96.0,
                ..parity_scene(12)
            },
            color: [0.711644, 0.818954, 1.0],
            environmental: Some(FogRamp {
                start: 10.0,
                end: 96.0,
            }),
            render_distance: TWELVE_CHUNKS,
            sky_end: 192.0,
            cloud_end: 2048.0,
        },
        ParityCase {
            name: "fog end at 512 blocks",
            inputs: FogInputs {
                fog_start_distance: 0.0,
                fog_end_distance: 512.0,
                ..parity_scene(12)
            },
            color: [0.711644, 0.818954, 1.0],
            environmental: Some(FogRamp {
                start: 0.0,
                end: 512.0,
            }),
            render_distance: TWELVE_CHUNKS,
            sky_end: 192.0,
            cloud_end: 2048.0,
        },
        ParityCase {
            name: "fog end above 512 blocks",
            inputs: FogInputs {
                fog_start_distance: 0.0,
                fog_end_distance: 512.1,
                ..parity_scene(12)
            },
            color: [0.711644, 0.818954, 1.0],
            environmental: Some(FogRamp::NONE),
            render_distance: TWELVE_CHUNKS,
            sky_end: 192.0,
            cloud_end: 2048.0,
        },
    ];

    for case in cases {
        assert_parity(&case);
    }
}

#[test]
fn sunrise_color_shows_toward_the_sun_from_four_chunks() {
    // With a black fog and sky, `C` is the share of the sunrise colour times `1 − k`.
    let share_at_four_chunks = 0.25 * (1.0 - 0.23429644);
    let cases = [
        SunriseCase {
            name: "sunset, looking west, r = 4",
            sun_angle: 90.0,
            view_east: -0.5,
            render_distance: 4,
            gray: share_at_four_chunks,
        },
        SunriseCase {
            name: "sunset, looking east, r = 4",
            sun_angle: 90.0,
            view_east: 0.5,
            render_distance: 4,
            gray: 0.0,
        },
        SunriseCase {
            name: "sunrise, looking east, r = 4",
            sun_angle: 270.0,
            view_east: 0.5,
            render_distance: 4,
            gray: share_at_four_chunks,
        },
        SunriseCase {
            name: "sunset, looking west, r = 3",
            sun_angle: 90.0,
            view_east: -0.5,
            render_distance: 3,
            gray: 0.0,
        },
    ];

    for case in cases {
        let inputs = FogInputs {
            fog_color: [0.0; 3],
            sky_color: [0.0; 3],
            sunrise_color: [1.0; 3],
            sunrise_alpha: 0.5,
            sun_angle: case.sun_angle,
            view_east: case.view_east,
            ..parity_scene(case.render_distance)
        };
        let fog = FogParameters::from_inputs(&inputs);
        for channel in fog.color {
            assert_near(case.name, "C", channel, case.gray, COLOR_TOLERANCE);
        }
    }
}
