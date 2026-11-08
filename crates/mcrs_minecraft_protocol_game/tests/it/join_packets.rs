//! Bytes written by the vanilla packet stream codecs; `id` lines are the registry
//! ids the capture session had.

use crate::common::encoded;
use std::collections::HashMap;
use std::sync::LazyLock;

use mcrs_minecraft_core::LookAnchor;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::OptGameMode;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::GlobalPos;
use mcrs_minecraft_protocol_game::clientbound::*;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};
use mcrs_voxel::BlockPos;

static GOLDEN: LazyLock<String> =
    LazyLock::new(|| mcrs_minecraft_fixtures::fixture("join_packets").text());

struct Fixture {
    ids: HashMap<(String, String), u16>,
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
            fixture.packets.insert(name.into(), crate::common::hex(hex));
        }
    }
    fixture
}

fn dimension_type(fixture: &Fixture, name: &str) -> Id<DimensionType> {
    let number = fixture.ids[&("dimension_type".into(), name.into())];
    Id::checked(number, usize::from(u16::MAX) + 1).unwrap()
}

fn decode<'a, P: Decode<'a>>(fixture: &'a Fixture, name: &str) -> (P, &'a [u8]) {
    let bytes = &fixture.packets[name][..];
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    (packet, bytes)
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

fn overworld() -> ResourceKeyBuf<Dimension> {
    mcrs_minecraft_dimension::dimension_keys::OVERWORLD.into()
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
            dimension_type_id: dimension_type(&fixture, "minecraft:overworld"),
            dimension: overworld(),
            game_mode: GameMode::Creative,
            prev_game_mode: OptGameMode(Some(GameMode::Survival)),
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

#[test]
fn respawn_equals_the_reference_bytes() {
    let fixture = fixture();
    let expected = ClientboundRespawn {
        player_spawn_info: PlayerSpawnInfo {
            dimension_type_id: dimension_type(&fixture, "minecraft:the_nether"),
            dimension: mcrs_minecraft_dimension::dimension_keys::THE_NETHER.into(),
            game_mode: GameMode::Survival,
            prev_game_mode: OptGameMode(None),
            is_flat: true,
            last_depth_location: Some(GlobalPos {
                dimension: overworld(),
                pos: BlockPos::new(1, 64, -3),
            }),
            portal_cooldown: VarInt(0),
            sea_level: VarInt(32),
        },
        data_to_keep: 3,
    };
    check(&fixture, "respawn", expected);
}

#[test]
fn abilities_and_look_at_follow_their_field_layouts() {
    let survival = ClientboundPlayerAbilities {
        invulnerable: false,
        flying: false,
        may_fly: false,
        instant_build: false,
        flying_speed: 0.05,
        walking_speed: 0.1,
    };
    let bytes = crate::common::hex("003d4ccccd3dcccccd");
    assert_eq!(encoded(&survival), bytes);
    let single_flags = [
        (
            0x01,
            ClientboundPlayerAbilities {
                invulnerable: true,
                ..survival
            },
        ),
        (
            0x02,
            ClientboundPlayerAbilities {
                flying: true,
                ..survival
            },
        ),
        (
            0x04,
            ClientboundPlayerAbilities {
                may_fly: true,
                ..survival
            },
        ),
        (
            0x08,
            ClientboundPlayerAbilities {
                instant_build: true,
                ..survival
            },
        ),
    ];
    for (flag, abilities) in single_flags {
        let mut bytes = bytes.clone();
        bytes[0] = flag;
        assert_eq!(encoded(&abilities), bytes, "{flag:#04x}");
        assert_eq!(
            ClientboundPlayerAbilities::decode(&mut &bytes[..]).unwrap(),
            abilities,
            "{flag:#04x}"
        );
    }

    let at_cow = ClientboundPlayerLookAt {
        from: LookAnchor::Feet,
        target: glam::DVec3::new(1.0, 2.0, 3.0),
        entity: Some(LookAtEntity {
            id: VarInt(5),
            anchor: LookAnchor::Eyes,
        }),
    };
    let mut expected = vec![0];
    for value in [1.0f64, 2.0, 3.0] {
        expected.extend(value.to_be_bytes());
    }
    expected.extend([1, 5, 1]);
    assert_eq!(encoded(&at_cow), expected);
    let at_point = ClientboundPlayerLookAt {
        from: LookAnchor::Eyes,
        entity: None,
        ..at_cow
    };
    assert_eq!(encoded(&at_point)[0], 1);
    assert_eq!(encoded(&at_point).last(), Some(&0));
}
