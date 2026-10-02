//! Bytes written by the vanilla packet stream codecs; `id` lines are the registry
//! ids the capture session had.

#[allow(dead_code)]
mod common;

use std::borrow::Cow;
use std::collections::HashMap;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::game_mode::OptGameMode;
use mcrs_minecraft_protocol::packets::game::clientbound::*;
use mcrs_minecraft_protocol::{Decode, Encode, GameMode, VarInt};

const GOLDEN: &str = include_str!("fixtures/join_packets_golden.txt");

struct Fixture {
    ids: HashMap<(String, String), u32>,
    packets: HashMap<String, Vec<u8>>,
}

fn fixture() -> Fixture {
    let mut fixture = Fixture {
        ids: HashMap::new(),
        packets: HashMap::new(),
    };
    for line in GOLDEN.lines() {
        if let Some(rest) = line.strip_prefix("id ") {
            let [registry, name, id] = rest.split(' ').collect::<Vec<_>>()[..] else {
                panic!("malformed id line: {line}");
            };
            fixture
                .ids
                .insert((registry.into(), name.into()), id.parse().unwrap());
        } else if let Some((name, hex)) = line.split_once(' ') {
            fixture.packets.insert(name.into(), common::hex(hex));
        }
    }
    fixture
}

fn id(fixture: &Fixture, registry: &str, name: &str) -> i32 {
    fixture.ids[&(registry.into(), name.into())] as i32
}

fn decode<'a, P: Decode<'a>>(fixture: &'a Fixture, name: &str) -> (P, &'a [u8]) {
    let bytes = &fixture.packets[name][..];
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    (packet, bytes)
}

fn encoded<P: Encode>(packet: &P) -> Vec<u8> {
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    out
}

fn check<'a, P: Encode + Decode<'a> + PartialEq + std::fmt::Debug>(
    fixture: &'a Fixture,
    name: &str,
    expected: P,
) {
    let (packet, bytes) = decode::<P>(fixture, name);
    assert_eq!(packet, expected, "{name}");
    assert_eq!(encoded(&expected), bytes, "{name}");
}

fn overworld() -> ResourceLocation<Cow<'static, str>> {
    ResourceLocation::from(mcrs_minecraft_core::rl!("minecraft:overworld"))
}

#[test]
fn login_carries_no_seed_and_equals_the_reference_bytes() {
    let fixture = fixture();
    let expected = ClientboundLogin {
        player_id: 7,
        hardcore: false,
        dimensions: vec![overworld()],
        max_players: VarInt(20),
        chunk_radius: VarInt(10),
        simulation_distance: VarInt(8),
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        player_spawn_info: PlayerSpawnInfo {
            dimension_type_id: VarInt(id(&fixture, "dimension_type", "minecraft:overworld")),
            dimension: overworld(),
            game_mode: GameMode::Creative,
            prev_game_mode: OptGameMode(Some(GameMode::Survival)),
            is_debug: false,
            is_flat: false,
            last_depth_location: None,
            portal_cooldown: VarInt(5),
            sea_level: VarInt(63),
        },
        online_mode: false,
        enforces_secure_chat: true,
    };
    check(&fixture, "login", expected);
}
