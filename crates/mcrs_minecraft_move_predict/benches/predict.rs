//! The cost of layer 2 for 10,000 scripted players that walk and sprint-jump on a floor, one
//! client tick each per server tick, on one core.

use std::hint::black_box;
use std::time::Duration;
use std::time::Instant;

use criterion::Criterion;
use criterion::criterion_group;
use glam::DVec3;
use mcrs_minecraft_move_predict::ClientTick;
use mcrs_minecraft_move_predict::Limits;
use mcrs_minecraft_move_predict::Prediction;
use mcrs_minecraft_move_predict::Pusher;
use mcrs_minecraft_move_validate::Move;
use mcrs_minecraft_physics::Keys;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerRotation;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::TestFixture;
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

fn floor() -> TestFixture {
    let mut fixture = TestFixture::new();
    let stone = fixture.blocks.stone;
    fixture.world.fill(
        BlockPos::new(-60, -1, -60),
        BlockPos::new(60, -1, 60),
        stone,
    );
    fixture.world.build_masks(&fixture.tables.collision);
    fixture
}

fn fresh(players: &[(PlayerState, Vec<ClientTick>)]) -> Vec<Prediction> {
    players
        .iter()
        .map(|(start, _)| Prediction::new(*start, Limits::default()))
        .collect()
}

fn server_tick(
    fixture: &TestFixture,
    predictions: &mut [Prediction],
    players: &[(PlayerState, Vec<ClientTick>)],
    tick: usize,
) {
    for (prediction, (_, ticks)) in predictions.iter_mut().zip(players) {
        let Some(client_tick) = ticks.get(tick) else {
            continue;
        };
        let outcome = prediction.tick(fixture.view(), &[] as &[Pusher], client_tick);
        assert!(outcome.offset.is_none(), "an honest client passes");
        black_box(outcome);
    }
}

fn predict(c: &mut Criterion) {
    let fixture = floor();
    let players = script(&fixture);
    let mut predictions = fresh(&players);
    let mut tick = 0;
    c.bench_function("predict, 10k scripted players, one server tick", |b| {
        b.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                if tick == TICKS {
                    tick = 0;
                    predictions = fresh(&players);
                }
                let started = Instant::now();
                server_tick(&fixture, &mut predictions, &players, tick);
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

/// `--ticks <n>` runs the first n server ticks once, for an instruction count under callgrind.
fn main() {
    let ticks = std::env::args().skip_while(|argument| argument != "--ticks").nth(1);
    let Some(ticks) = ticks.and_then(|ticks| ticks.parse::<usize>().ok()) else {
        benches();
        Criterion::default().configure_from_args().final_summary();
        return;
    };
    let fixture = floor();
    let players = script(&fixture);
    let mut predictions = fresh(&players);
    for tick in 0..ticks {
        server_tick(&fixture, &mut predictions, &players, tick);
    }
}
