//! The controls against GOAL-CTL and PATH-MOV, on a walker that moves along its yaw at the steady
//! speed on grass of its input.

use glam::DVec3;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::LookIntent;
use mcrs_minecraft_brain::PathIntent;
use mcrs_minecraft_brain::Progress;
use mcrs_minecraft_brain::Ticket;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::MobData;
use mcrs_minecraft_mob_control::BodyData;
use mcrs_minecraft_mob_control::BodyFlags;
use mcrs_minecraft_mob_control::ControlInput;
use mcrs_minecraft_mob_control::ControlOutput;
use mcrs_minecraft_mob_control::LookDefaults;
use mcrs_minecraft_mob_control::MobControl;
use mcrs_minecraft_mob_control::PathEnd;
use mcrs_minecraft_mob_control::Rotations;
use mcrs_minecraft_numeric::wrap_degrees;

const PIG_LOOK: LookDefaults = LookDefaults {
    turn_speed: MobData::DEFAULT.head_turn_speed,
    pitch_limit: MobData::DEFAULT.pitch_limit,
};

const PIG: BodyData = BodyData {
    width: EntityTypeData::PIG.width,
    eye_height: EntityTypeData::PIG.eye_height,
    step_height: 0.6,
    head_yaw_limit: MobData::DEFAULT.head_yaw_limit,
};

struct Walker {
    control: MobControl,
    input: ControlInput,
    moves: bool,
}

impl Walker {
    fn new(position: DVec3) -> Self {
        Self {
            control: MobControl::new(PIG_LOOK),
            input: ControlInput {
                tick: 0,
                position,
                rotations: Rotations::default(),
                body: PIG,
                flags: BodyFlags::ON_GROUND,
                sideways: 0.0,
                speed: 0.0,
                movement_speed: 0.25,
                feet_block_top: None,
            },
            moves: true,
        }
    }

    fn apply(&mut self, intents: Intents) {
        self.control.apply(&intents, self.input.position);
    }

    /// One tick: the controls, a travel along the yaw at `forward × speed × 2.2026` blocks, and
    /// the body rotation.
    fn tick(&mut self) -> ControlOutput {
        let output = self.control.before_travel(&self.input);
        let input = &mut self.input;
        let mut rotations = input.rotations;
        rotations.yaw = output.yaw.unwrap_or(rotations.yaw);
        rotations.pitch = output.pitch.unwrap_or(rotations.pitch);
        rotations.head_yaw = output.head_yaw.unwrap_or(rotations.head_yaw);
        let (forward, speed) = output
            .steering
            .map_or((0.0, 0.0), |steering| (steering.forward, steering.speed));
        input.speed = speed;
        let step = if self.moves {
            f64::from(forward * speed) * 2.2026
        } else {
            0.0
        };
        let radians = f64::from(rotations.yaw).to_radians();
        let end = DVec3::new(
            input.position.x - radians.sin() * step,
            input.position.y,
            input.position.z + radians.cos() * step,
        );
        input.rotations = self.control.after_travel(input, end, rotations);
        input.position = end;
        input.tick += 1;
        output
    }
}

fn walk_to(target: DVec3) -> Intents {
    Intents {
        path: Some(PathIntent::Walk {
            ticket: Ticket(1),
            target,
            speed_modifier: 1.0,
        }),
        ..Intents::default()
    }
}

#[test]
fn fol_006_repeated_walks_keep_the_stuck_window_when_tickets_and_speed_change() {
    let mut walker = Walker::new(DVec3::ZERO);
    walker.moves = false;
    walker.input.speed = 0.25;
    for tick in 1..=101 {
        walker.apply(Intents {
            path: Some(PathIntent::Walk {
                ticket: Ticket(tick),
                target: DVec3::new(10.0, 0.0, 0.0),
                speed_modifier: if tick < 50 { 1.0 } else { 2.0 },
            }),
            ..Intents::default()
        });
        let output = walker.tick();
        let expected = (tick == 101).then_some(PathEnd::Stuck);
        assert_eq!(output.path_ended, expected, "follower tick {tick}");
        assert_eq!(walker.control.applied().ticket, Ticket(tick));
        if tick == 50 {
            assert_eq!(output.steering.unwrap().speed, 0.5);
        }
    }
    assert_eq!(walker.control.applied().progress, Progress::Ended);
}

#[test]
fn fol_004_repeated_walks_keep_the_timeout_for_the_current_node() {
    let mut walker = Walker::new(DVec3::ZERO);
    walker.moves = false;
    walker.input.speed = 0.25;
    let target = DVec3::new(10.0, 0.0, 0.0);
    walker.apply(walk_to(target));
    assert_eq!(walker.tick().path_ended, None);
    walker.input.tick = 10_000;
    walker.apply(walk_to(target));
    assert_eq!(walker.tick().path_ended, Some(PathEnd::TimedOut));
}

#[test]
fn req_004_a_changed_node_restarts_the_stuck_window() {
    for target in [
        DVec3::new(20.0, 0.0, 0.0),
        DVec3::new(10.0, 1.0, 0.0),
        DVec3::new(10.0, 0.0, 10.0),
    ] {
        let mut walker = Walker::new(DVec3::ZERO);
        walker.moves = false;
        walker.input.speed = 0.25;
        walker.apply(walk_to(DVec3::new(10.0, 0.0, 0.0)));
        for _ in 0..100 {
            assert_eq!(walker.tick().path_ended, None);
        }
        walker.apply(walk_to(target));
        for _ in 0..100 {
            assert_eq!(walker.tick().path_ended, None, "target {target}");
        }
        assert_eq!(walker.tick().path_ended, Some(PathEnd::Stuck));
    }
}

#[test]
fn fol_003_zero_speed_does_not_report_stuck() {
    let mut walker = Walker::new(DVec3::ZERO);
    walker.moves = false;
    walker.input.movement_speed = 0.0;
    for _ in 0..101 {
        walker.apply(walk_to(DVec3::new(10.5, 0.0, 0.5)));
        assert_eq!(walker.tick().path_ended, None);
    }
    assert!(walker.control.has_path(), "zero speed must keep the route");
}

#[test]
fn fol_003_reaching_the_node_precedes_the_stuck_check_at_tick_101() {
    let mut walker = Walker::new(DVec3::ZERO);
    walker.moves = false;
    let target = DVec3::new(10.5, 0.0, 0.5);
    for _ in 0..100 {
        walker.apply(walk_to(target));
        assert_eq!(walker.tick().path_ended, None);
    }
    walker.input.position = target;
    walker.apply(walk_to(target));
    assert_eq!(walker.tick().path_ended, Some(PathEnd::Done));
}

#[test]
fn a_walker_reaches_its_target_and_stops() {
    let mut walker = Walker::new(DVec3::new(0.5, 0.0, 0.5));
    walker.apply(walk_to(DVec3::new(8.5, 0.0, 3.5)));
    assert_eq!(walker.control.applied().progress, Progress::Running);
    let distance = (8.0_f64 * 8.0 + 3.0 * 3.0).sqrt();
    let steady = 0.25 * 0.25 * 2.2026;
    let mut ended = None;
    for tick in 0..200 {
        if let Some(end) = walker.tick().path_ended {
            ended = Some((tick, end));
            break;
        }
    }
    let (tick, end) = ended.expect("the path ends");
    assert_eq!(end, PathEnd::Done);
    assert!(
        f64::from(tick) <= (distance / steady).ceil() + 3.0,
        "the walker heads straight to the target at the stroll speed 0.1377; ended at {tick}"
    );
    let position = walker.input.position;
    assert!((position.x - 8.5).abs() < 0.45 && (position.z - 3.5).abs() < 0.45);
    assert_eq!(walker.control.applied().progress, Progress::Ended);
    assert_eq!(
        walker.tick().steering.unwrap().forward,
        0.0,
        "with no path the move control waits"
    );
}

#[test]
fn the_move_control_turns_the_yaw_by_at_most_ninety_degrees_in_a_tick() {
    let mut walker = Walker::new(DVec3::new(0.5, 0.0, 0.5));
    walker.moves = false;
    walker.apply(walk_to(DVec3::new(0.5, 0.0, -9.5)));
    let first = walker.tick().yaw.unwrap();
    assert_eq!(
        wrap_degrees(first - 0.0).abs(),
        90.0,
        "the target is behind the mob"
    );
    let second = walker.tick().yaw.unwrap();
    assert!(
        wrap_degrees(second - 180.0).abs() < 1e-3,
        "the second tick faces the target: {second}"
    );
}

#[test]
fn the_head_follows_a_look_target_within_its_speed_and_limits() {
    let mut walker = Walker::new(DVec3::new(0.5, 0.0, 0.5));
    walker.moves = false;
    walker.apply(Intents {
        look: Some(LookIntent::At {
            point: DVec3::new(-20.5, 30.0, 0.5),
            turn_speed: Some(10.0),
            pitch_limit: Some(40.0),
            hold: 12,
        }),
        ..Intents::default()
    });
    let mut previous = 0.0;
    for tick in 0..12 {
        let output = walker.tick();
        let head = output.head_yaw.unwrap();
        assert!(
            wrap_degrees(head - previous).abs() <= 10.0 + 1e-4,
            "tick {tick}"
        );
        assert_eq!(
            output.pitch,
            Some(-40.0),
            "the point is far above: the pitch is clamped"
        );
        previous = head;
    }
    assert!(
        wrap_degrees(previous - 90.0).abs() < 1e-3,
        "the yaw to -x is 90 degrees: {previous}"
    );
    let output = walker.tick();
    assert_eq!(
        output.pitch,
        Some(0.0),
        "with no hold the pitch is 0 at once"
    );
    assert!(
        (wrap_degrees(output.head_yaw.unwrap() - previous).abs() - 10.0).abs() < 1e-3,
        "with no hold the head returns to the body at 10 degrees in a tick"
    );
}

#[test]
fn a_standing_body_follows_a_still_head_after_ten_ticks() {
    let mut walker = Walker::new(DVec3::new(0.5, 0.0, 0.5));
    walker.moves = false;
    walker.input.rotations.head_yaw = 60.0;
    walker.apply(Intents {
        look: Some(LookIntent::At {
            point: DVec3::new(
                -9.5,
                0.765,
                0.5 + 10.0 * (-30.0_f64).to_radians().tan().abs(),
            ),
            turn_speed: Some(10.0),
            pitch_limit: Some(40.0),
            hold: 200,
        }),
        ..Intents::default()
    });
    let mut bodies = Vec::new();
    for _ in 0..40 {
        walker.tick();
        bodies.push(walker.input.rotations.body_yaw);
    }
    let head = walker.input.rotations.head_yaw;
    assert!(bodies.iter().take(10).all(|body| *body == bodies[0]));
    assert!(
        wrap_degrees(head - bodies[39]).abs() < 1e-3,
        "the body faces the head within 20 ticks of a still head: {head} against {}",
        bodies[39]
    );
}

#[test]
fn a_walker_that_does_not_move_drops_its_path_after_the_stuck_check() {
    let mut walker = Walker::new(DVec3::new(0.5, 0.0, 0.5));
    walker.moves = false;
    walker.apply(walk_to(DVec3::new(10.5, 0.0, 0.5)));
    let ended = (1..=200).find_map(|tick| walker.tick().path_ended.map(|end| (tick, end)));
    assert_eq!(
        ended,
        Some((101, PathEnd::Stuck)),
        "the check fires after 101 ticks"
    );
}

#[test]
fn a_walking_head_stays_within_the_head_yaw_limit_of_the_body() {
    let mut walker = Walker::new(DVec3::new(0.5, 0.0, 0.5));
    let mut intents = walk_to(DVec3::new(0.5, 0.0, 20.5));
    intents.look = Some(LookIntent::At {
        point: DVec3::new(0.5, 0.765, -30.5),
        turn_speed: Some(10.0),
        pitch_limit: Some(40.0),
        hold: 100,
    });
    walker.apply(intents);
    let mut widest: f32 = 0.0;
    for _ in 0..60 {
        let output = walker.tick();
        let body = walker.input.rotations.body_yaw;
        widest = widest.max(wrap_degrees(output.head_yaw.unwrap() - body).abs());
    }
    assert!(
        (widest - 75.0).abs() < 1e-3,
        "the head turns to the target behind as far as the limit allows: {widest}"
    );
}
