//! The cost of layer 2 for 10,000 scripted players that walk and sprint-jump on a floor, one
//! client tick each per server tick, on one core.

use std::hint::black_box;
use std::time::Duration;
use std::time::Instant;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use glam::DVec3;
use mcrs_minecraft_move_predict::ClientTick;
use mcrs_minecraft_move_predict::Limits;
use mcrs_minecraft_move_predict::Prediction;
use mcrs_minecraft_move_predict::Pusher;
use mcrs_minecraft_move_validate::Move;
use mcrs_minecraft_physics::Keys;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerRotation;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;

const PLAYERS: usize = 10_000;
const TICKS: usize = 40;

fn keys(tick: usize) -> Keys {
    Keys {
        forward: true,
        jump: tick % 10 < 4,
        sprint: tick >= 10,
        ..Keys::default()
    }
}

/// The start state and the client ticks of each player, as an honest client reports them.
fn script(fixture: &TestFixture) -> Vec<(PlayerState, Vec<ClientTick>)> {
    let mut actions = Vec::new();
    let mut scratch = Scratch::new();
    (0..PLAYERS)
        .map(|index| {
            let x = -50.0 + (index % 100) as f64 * 0.9;
            let z = -50.0 + (index / 100) as f64;
            let start = PlayerState::new(DVec3::new(x, 0.0, z));
            let mut client = start;
            let ticks = (0..TICKS)
                .map(|tick| {
                    let input = PlayerInput {
                        keys: keys(tick),
                        rotation: PlayerRotation {
                            yaw: -90.0 + index as f32 % 7.0,
                            pitch: 0.0,
                        },
                        ..PlayerInput::default()
                    };
                    let before = client;
                    tick_player(
                        fixture.view(),
                        Side::OwnClient,
                        &before,
                        &input,
                        &mut client,
                        &mut actions,
                        &mut scratch,
                    );
                    actions.clear();
                    ClientTick {
                        input,
                        reported: Move {
                            position: Some(client.position),
                            look: None,
                            on_ground: client.flags.on_ground,
                            horizontal_collision: client.flags.horizontal_collision,
                        },
                        ..ClientTick::default()
                    }
                })
                .collect();
            (start, ticks)
        })
        .collect()
}

fn predict(c: &mut Criterion) {
    let mut fixture = TestFixture::new();
    let stone = fixture.blocks.stone;
    fixture.world.fill(
        BlockPos::new(-60, -1, -60),
        BlockPos::new(60, -1, 60),
        stone,
    );
    fixture.world.build_masks(&fixture.tables.collision);
    let players = script(&fixture);
    let fresh = || -> Vec<Prediction> {
        players
            .iter()
            .map(|(start, _)| Prediction::new(*start, Limits::default()))
            .collect()
    };
    let mut predictions = fresh();
    let mut tick = 0;
    let view = View::new(&fixture.world, &NoBodies, &fixture.tables);
    c.bench_function("predict, 10k scripted players, one server tick", |b| {
        b.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                if tick == TICKS {
                    tick = 0;
                    predictions = fresh();
                }
                let started = Instant::now();
                for (prediction, (_, ticks)) in predictions.iter_mut().zip(&players) {
                    let Some(client_tick) = ticks.get(tick) else {
                        continue;
                    };
                    let outcome = prediction.tick(view, &[] as &[Pusher], client_tick);
                    assert!(outcome.offset.is_none(), "an honest client passes");
                    black_box(outcome);
                }
                elapsed += started.elapsed();
                tick += 1;
            }
            elapsed
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20).measurement_time(Duration::from_secs(15));
    targets = predict
}
criterion_main!(benches);
