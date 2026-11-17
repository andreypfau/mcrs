use std::time::Duration;
use std::time::Instant;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use glam::DVec3;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::LookIntent;
use mcrs_minecraft_brain::PathIntent;
use mcrs_minecraft_brain::Ticket;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::MobData;
use mcrs_minecraft_mob_control::BodyData;
use mcrs_minecraft_mob_control::BodyFlags;
use mcrs_minecraft_mob_control::ControlInput;
use mcrs_minecraft_mob_control::ControlOutput;
use mcrs_minecraft_mob_control::LookDefaults;
use mcrs_minecraft_mob_control::MobControl;
use mcrs_minecraft_mob_control::Rotations;

const MOBS: u32 = 50_000;
const PIG_LOOK: LookDefaults = LookDefaults {
    turn_speed: MobData::DEFAULT.head_turn_speed,
    pitch_limit: MobData::DEFAULT.pitch_limit,
};

struct Mob {
    control: MobControl,
    input: ControlInput,
}

fn mobs() -> Vec<Mob> {
    (0..MOBS)
        .map(|index| {
            let position = DVec3::new(
                f64::from(index % 250) * 4.0 + 0.5,
                0.0,
                f64::from(index / 250) * 4.0 + 0.5,
            );
            let mut control = MobControl::new(PIG_LOOK);
            let intents = match index % 3 {
                0 => Intents {
                    path: Some(PathIntent::Walk {
                        ticket: Ticket(1),
                        target: DVec3::new(position.x + 8.0, 0.0, position.z + 3.0),
                        speed_modifier: 1.0,
                    }),
                    ..Intents::default()
                },
                1 => Intents {
                    look: Some(LookIntent::Around { cos: 0.6, sin: 0.8 }),
                    ..Intents::default()
                },
                _ => Intents {
                    look: Some(LookIntent::At {
                        point: DVec3::new(position.x - 3.0, 1.6, position.z + 2.0),
                        turn_speed: Some(10.0),
                        pitch_limit: Some(40.0),
                        hold: u8::MAX,
                    }),
                    ..Intents::default()
                },
            };
            control.apply(&intents, position);
            let input = ControlInput {
                tick: 0,
                position,
                rotations: Rotations::default(),
                body: BodyData {
                    width: EntityTypeData::PIG.width,
                    eye_height: EntityTypeData::PIG.eye_height,
                    step_height: 0.6,
                    head_yaw_limit: MobData::DEFAULT.head_yaw_limit,
                },
                flags: BodyFlags::ON_GROUND,
                sideways: 0.0,
                speed: 0.0,
                movement_speed: 0.25,
                feet_block_top: None,
            };
            Mob { control, input }
        })
        .collect()
}

fn tick(mobs: &mut [Mob], outputs: &mut Vec<ControlOutput>, ends: &mut Vec<DVec3>) -> Duration {
    let start = Instant::now();
    outputs.clear();
    outputs.extend(
        mobs.iter_mut()
            .map(|mob| mob.control.before_travel(&mob.input)),
    );
    let mut timed = start.elapsed();
    ends.clear();
    for (mob, output) in mobs.iter_mut().zip(outputs.iter()) {
        let input = &mut mob.input;
        let rotations = &mut input.rotations;
        rotations.yaw = output.yaw.unwrap_or(rotations.yaw);
        rotations.pitch = output.pitch.unwrap_or(rotations.pitch);
        rotations.head_yaw = output.head_yaw.unwrap_or(rotations.head_yaw);
        let (forward, speed) = output
            .steering
            .map_or((0.0, 0.0), |steering| (steering.forward, steering.speed));
        input.speed = speed;
        let step = f64::from(forward * speed) * 2.2026;
        let radians = f64::from(rotations.yaw).to_radians();
        ends.push(DVec3::new(
            input.position.x - radians.sin() * step,
            input.position.y,
            input.position.z + radians.cos() * step,
        ));
    }
    let start = Instant::now();
    for (mob, end) in mobs.iter_mut().zip(ends.iter()) {
        let rotations = mob
            .control
            .after_travel(&mob.input, *end, mob.input.rotations);
        mob.input.rotations = rotations;
    }
    timed += start.elapsed();
    for (mob, end) in mobs.iter_mut().zip(ends.iter()) {
        mob.input.position = *end;
        mob.input.tick += 1;
    }
    timed
}

fn controls(criterion: &mut Criterion) {
    let mut outputs = Vec::new();
    let mut ends = Vec::new();
    criterion.bench_function("controls of 50k mobs, 1 thread", |bencher| {
        bencher.iter_custom(|iterations| {
            let mut fresh = mobs();
            (0..iterations)
                .map(|_| tick(&mut fresh, &mut outputs, &mut ends))
                .sum()
        });
    });
}

criterion_group!(benches, controls);
criterion_main!(benches);
