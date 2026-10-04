//! Bytes written by the vanilla packet stream codecs for the serverbound game packets.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use bevy_math::DVec3;
use mcrs_minecraft_core::{Bounded, ResourceLocation, rl};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::serverbound::{
    CustomClickAction, Payload, Pong, ResourcePack,
};
use mcrs_minecraft_protocol::packets::cookie::serverbound::CookieResponse;
use mcrs_minecraft_protocol::packets::game::serverbound::*;
use mcrs_minecraft_protocol::resource_pack::Status;
use mcrs_minecraft_protocol::{Decode, Encode, Hand, Look, LpVec3, Position, VarInt};
use uuid::Uuid;

const GOLDEN: &str = include_str!("../fixtures/serverbound_game_packets_golden.txt");

fn labels() -> &'static BTreeMap<&'static str, Vec<u8>> {
    static LABELS: OnceLock<BTreeMap<&'static str, Vec<u8>>> = OnceLock::new();
    LABELS.get_or_init(|| {
        GOLDEN
            .lines()
            .map(|line| {
                let (name, hex) = line.split_once(' ').expect("a label and its bytes");
                (name, crate::common::hex(hex))
            })
            .collect()
    })
}

fn bytes(name: &str) -> &'static [u8] {
    labels()
        .get(name)
        .unwrap_or_else(|| panic!("the golden has no label {name}"))
}

fn encoded<P: Encode>(packet: &P) -> Vec<u8> {
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    out
}

fn check<P>(name: &str, expected: P)
where
    P: Encode + PartialEq + std::fmt::Debug + Decode<'static>,
{
    let bytes = bytes(name);
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    assert_eq!(packet, expected, "{name}");
    assert_eq!(encoded(&expected), bytes, "{name}");
}

fn refused<P>(bytes: &[u8])
where
    P: for<'a> Decode<'a> + std::fmt::Debug,
{
    let mut r = bytes;
    let decoded = P::decode(&mut r);
    assert!(decoded.is_err(), "decoded {decoded:?} from {bytes:02x?}");
}

fn leaves_one_byte_unread<P>(name: &str)
where
    P: for<'a> Decode<'a>,
{
    let mut padded = bytes(name).to_vec();
    padded.push(0xa5);
    let mut r = &padded[..];
    P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert_eq!(r, [0xa5], "{name} swallowed or left the wrong bytes");
}

fn flags(spec: &[bool; 7]) -> PlayerInputFlags {
    PlayerInputFlags::new()
        .with_forward(spec[0])
        .with_backward(spec[1])
        .with_left(spec[2])
        .with_right(spec[3])
        .with_jump(spec[4])
        .with_shift(spec[5])
        .with_sprint(spec[6])
}

#[test]
fn every_packet_decodes_the_games_bytes() {
    check(
        "attack",
        ServerboundAttack {
            entity_id: VarInt(300),
        },
    );
    check(
        "client_command",
        ServerboundClientCommand {
            action: ClientCommandAction::RequestStats,
        },
    );
    check(
        "client_command_last",
        ServerboundClientCommand {
            action: ClientCommandAction::RequestGameRuleValues,
        },
    );
    check("client_tick_end", ServerboundClientTickEnd);
    check(
        "interact",
        ServerboundInteract {
            entity_id: VarInt(300),
            hand: Hand::Off,
            location: LpVec3(DVec3::new(1.0, 0.0, -1.0)),
            using_secondary_action: true,
        },
    );
    check(
        "move_vehicle",
        ServerboundMoveVehicle {
            position: Position::new(1.5, 64.0, -2.5),
            look: Look {
                yaw: 90.0,
                pitch: -15.5,
            },
            on_ground: true,
        },
    );
    check(
        "player_abilities",
        ServerboundPlayerAbilities { flying: true },
    );
    check(
        "player_abilities_grounded",
        ServerboundPlayerAbilities { flying: false },
    );
    check(
        "player_command",
        ServerboundPlayerCommand {
            entity_id: VarInt(300),
            action: PlayerCommandAction::StopSprinting,
            data: VarInt(0),
        },
    );
    check(
        "player_command_last",
        ServerboundPlayerCommand {
            entity_id: VarInt(300),
            action: PlayerCommandAction::StartFallFlying,
            data: VarInt(1000),
        },
    );
    check(
        "player_input",
        ServerboundPlayerInput {
            input: flags(&[true, false, false, false, true, false, true]),
        },
    );
    check(
        "player_input_none",
        ServerboundPlayerInput {
            input: flags(&[false; 7]),
        },
    );
    check(
        "player_input_all",
        ServerboundPlayerInput {
            input: flags(&[true; 7]),
        },
    );
    check("player_loaded", ServerboundPlayerLoaded);
    check("punch", ServerboundPunch);
    check(
        "use_item",
        ServerboundUseItem {
            hand: Hand::Off,
            sequence: VarInt(300),
            look: Look {
                yaw: 90.0,
                pitch: -15.5,
            },
        },
    );
    {
        let key = ResourceLocation::from(rl!("minecraft:session"));
        check(
            "cookie_response",
            ServerboundCookieResponse(CookieResponse {
                key: key.clone(),
                payload: Some(Bounded(&[1, 2, 3][..])),
            }),
        );
        check(
            "cookie_response_absent",
            ServerboundCookieResponse(CookieResponse { key, payload: None }),
        );
    }
    check(
        "custom_payload",
        ServerboundCustomPayload(Payload::Brand(Brand { brand: "vanilla" })),
    );
    check("pong", ServerboundPong(Pong { payload: -1234 }));
    check(
        "resource_pack",
        ServerboundResourcePack(ResourcePack {
            id: Uuid::from_u64_pair(0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210),
            status: Status::Discarded,
        }),
    );
    {
        let id = ResourceLocation::from(rl!("example:action"));
        let mut payload = NbtCompound::new();
        payload.put_string("name", "ok".into());
        payload.put_int("count", 3);
        check(
            "custom_click_action",
            ServerboundCustomClickAction(CustomClickAction {
                id: id.clone(),
                payload: Some(NbtTag::Compound(payload)),
            }),
        );
        check(
            "custom_click_action_absent",
            ServerboundCustomClickAction(CustomClickAction { id, payload: None }),
        );
    }
}

#[test]
fn an_action_past_the_last_is_refused() {
    refused::<ServerboundClientCommand>(&[3]);
    refused::<ServerboundPlayerCommand>(&[1, 7, 0]);
}

#[test]
fn the_abilities_byte_reads_only_the_flying_bit() {
    let foreign = bytes("player_abilities_foreign_bits");
    assert_eq!(foreign, bytes("player_abilities"));
    for byte in [0xffu8, 0xfe, 0x02, 0x03] {
        let mut r = &[byte][..];
        let decoded = ServerboundPlayerAbilities::decode(&mut r).unwrap();
        assert!(decoded.flying, "{byte:#04x}");
        assert_eq!(encoded(&decoded), foreign, "{byte:#04x}");
    }
    for byte in [0x00u8, 0xfd, 0x01, 0x7d] {
        let mut r = &[byte][..];
        let decoded = ServerboundPlayerAbilities::decode(&mut r).unwrap();
        assert!(!decoded.flying, "{byte:#04x}");
        assert_eq!(
            encoded(&decoded),
            bytes("player_abilities_grounded"),
            "{byte:#04x}"
        );
    }
}

#[test]
fn a_typed_packet_leaves_a_trailing_byte_unread() {
    leaves_one_byte_unread::<ServerboundAttack>("attack");
    leaves_one_byte_unread::<ServerboundClientCommand>("client_command");
    leaves_one_byte_unread::<ServerboundClientTickEnd>("client_tick_end");
    leaves_one_byte_unread::<ServerboundInteract>("interact");
    leaves_one_byte_unread::<ServerboundMoveVehicle>("move_vehicle");
    leaves_one_byte_unread::<ServerboundPlayerAbilities>("player_abilities");
    leaves_one_byte_unread::<ServerboundPlayerCommand>("player_command");
    leaves_one_byte_unread::<ServerboundPlayerInput>("player_input");
    leaves_one_byte_unread::<ServerboundPlayerLoaded>("player_loaded");
    leaves_one_byte_unread::<ServerboundPunch>("punch");
    leaves_one_byte_unread::<ServerboundUseItem>("use_item");
}
