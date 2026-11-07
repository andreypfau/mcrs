//! The cost of the layers per packet, on one core with a warm cache.

use std::hint::black_box;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use mcrs_minecraft_move_validate::check_serverbound;
use mcrs_minecraft_protocol::packets::table::game_serverbound::NAMES;
use mcrs_minecraft_protocol::packets::table::row_of;

fn id(name: &str) -> i32 {
    row_of(NAMES, name).unwrap_or(-1)
}

/// The packets of one client tick of a walking player.
fn tick_packets() -> [(i32, Vec<u8>); 3] {
    let position: Vec<u8> = [0.5f64, 64.0, -0.5]
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .chain([10.0f32, 20.0].iter().flat_map(|value| value.to_be_bytes()))
        .chain([1])
        .collect();
    [
        (id("player_input"), vec![0x01]),
        (id("move_player_pos_rot"), position),
        (id("client_tick_end"), Vec::new()),
    ]
}

fn layer_0(c: &mut Criterion) {
    let packets = tick_packets();
    c.bench_function("layer 0, the three packets of a walking tick", |b| {
        b.iter(|| {
            for (id, body) in &packets {
                black_box(check_serverbound(black_box(*id), black_box(body)));
            }
        });
    });
}

criterion_group!(layers, layer_0);
criterion_main!(layers);
