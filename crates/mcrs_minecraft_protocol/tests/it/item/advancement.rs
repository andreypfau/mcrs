use crate::item::harness::WireCodec;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_protocol::advancement::{
    Advancement, AdvancementProgress, AdvancementType, CriterionProgress, DisplayInfo,
    PositionedAdvancement,
};
use mcrs_minecraft_protocol::item::{ComponentPatch, Template, Text};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundUpdateAdvancements;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    SeenAdvancementsAction, ServerboundSeenAdvancements,
};
use mcrs_minecraft_protocol::{Decode, Encode};
use mcrs_minecraft_registries::FromWire;

use crate::item::harness::{block_lookup, hex, json_of, read_json};
use crate::item::particle::{golden, to_hex};

fn id(text: &str) -> ResourceLocation {
    ResourceLocation::read(text).unwrap()
}

fn item(path: &str, count: i32, components: &str) -> Template {
    Template::new(
        Item::read(&format!("minecraft:{path}")).unwrap().id(),
        count,
        read_json::<ComponentPatch>(components).unwrap(),
    )
    .unwrap()
}

fn root() -> Advancement {
    Advancement {
        parent: None,
        display: Some(DisplayInfo {
            icon: item("apple", 1, "{}"),
            title: Text::text("Root"),
            description: Text::translate("adv.desc", vec![]),
            background: Some(id("minecraft:gui/advancements/backgrounds/stone")),
            frame: AdvancementType::Challenge,
            show_toast: true,
            announce_to_chat: false,
            hidden: false,
        }),
        requirements: vec![vec!["a".into(), "b".into()], vec!["c".into()]],
        sends_telemetry_event: true,
    }
}

fn child() -> Advancement {
    Advancement {
        parent: Some(id("mcrs:root")),
        display: Some(DisplayInfo {
            icon: item("diamond_sword", 2, r#"{"max_stack_size":16}"#),
            title: Text::text("Child"),
            description: Text::text("hidden one"),
            background: None,
            frame: AdvancementType::Goal,
            show_toast: false,
            announce_to_chat: false,
            hidden: true,
        }),
        requirements: vec![],
        sends_telemetry_event: false,
    }
}

fn bare() -> Advancement {
    Advancement {
        parent: Some(id("mcrs:root")),
        display: None,
        requirements: vec![vec!["x".into()]],
        sends_telemetry_event: false,
    }
}

fn positioned(name: &str, value: Advancement, x: f32, y: f32) -> PositionedAdvancement {
    PositionedAdvancement {
        id: id(name),
        advancement: value,
        x,
        y,
    }
}

#[test]
fn an_advancement_matches_the_vanilla_stream_codec() {
    let bytes = hex(golden("root.wire"));
    let mut r = &bytes[..];
    let decoded = Advancement::decode_checked(&block_lookup(), &mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(decoded, root());
    let mut out = Vec::new();
    decoded.encode_checked(&block_lookup(), &mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));
}

#[test]
fn display_info_matches_the_vanilla_codec() {
    let parsed: DisplayInfo = read_json(golden("root_display.json")).unwrap();
    let mut expected = root().display.unwrap();
    expected.announce_to_chat = true;
    assert_eq!(parsed, expected);
    let back: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(&json_of(&parsed)).unwrap();
    let vanilla: serde_json::Value = serde_json::from_str(golden("root_display.json")).unwrap();
    assert_eq!(back, vanilla);

    let parsed: DisplayInfo = read_json(golden("child_display.json")).unwrap();
    assert_eq!(parsed, child().display.unwrap());
    let back: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(&json_of(&parsed)).unwrap();
    let vanilla: serde_json::Value = serde_json::from_str(golden("child_display.json")).unwrap();
    assert_eq!(back, vanilla);

    let defaults: DisplayInfo =
        read_json(r#"{"icon":"minecraft:apple","title":"t","description":"d"}"#).unwrap();
    assert_eq!(defaults.frame, AdvancementType::Task);
    assert!(defaults.show_toast && defaults.announce_to_chat && !defaults.hidden);
    assert_eq!(
        json_of(&defaults),
        r#"{"icon":{"id":"minecraft:apple"},"title":"t","description":"d"}"#
    );
}

#[test]
fn update_advancements_packet_matches_vanilla() {
    let bytes = hex(golden("advancements.wire"));
    let mut r = &bytes[..];
    let packet = ClientboundUpdateAdvancements::decode(&mut r).unwrap();
    assert!(r.is_empty());
    assert!(packet.reset);
    assert!(packet.show_advancements);
    assert_eq!(
        packet.removed,
        vec![id("minecraft:story/root"), id("mcrs:gone")]
    );
    assert_eq!(
        packet.progress,
        vec![
            (
                id("mcrs:root"),
                AdvancementProgress {
                    criteria: vec![
                        (
                            "a".into(),
                            CriterionProgress {
                                obtained: Some(1700000000123),
                            }
                        ),
                        ("b".into(), CriterionProgress { obtained: None }),
                    ],
                }
            ),
            (id("mcrs:child"), AdvancementProgress::default()),
        ]
    );
    let resolved: Vec<PositionedAdvancement> = packet
        .added
        .iter()
        .map(|wire| PositionedAdvancement::from_wire(wire.clone(), &block_lookup()).unwrap())
        .collect();
    assert_eq!(
        resolved,
        vec![
            positioned("mcrs:root", root(), 0.5, 1.5),
            positioned("mcrs:child", child(), 2.0, -1.0),
            positioned("mcrs:bare", bare(), 0.0, 0.0),
        ]
    );
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));

    let rebuilt = ClientboundUpdateAdvancements {
        added: resolved
            .iter()
            .map(|advancement| advancement.to_wire(&block_lookup()).unwrap())
            .collect(),
        ..packet
    };
    let mut out = Vec::new();
    rebuilt.encode(&mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));

    let empty = hex(golden("advancements_empty.wire"));
    let packet = ClientboundUpdateAdvancements::decode(&mut &empty[..]).unwrap();
    assert_eq!(
        packet,
        ClientboundUpdateAdvancements {
            reset: false,
            added: vec![],
            removed: vec![],
            progress: vec![],
            show_advancements: false,
        }
    );
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    assert_eq!(out, empty);
}

#[test]
fn seen_advancements_matches_vanilla() {
    for (label, action) in [
        (
            "seen_opened",
            SeenAdvancementsAction::OpenedTab(id("mcrs:root")),
        ),
        ("seen_closed", SeenAdvancementsAction::ClosedScreen),
    ] {
        let bytes = hex(golden(&format!("{label}.wire")));
        let mut r = &bytes[..];
        let packet = ServerboundSeenAdvancements::decode(&mut r).unwrap();
        assert!(r.is_empty());
        assert_eq!(packet.action, action);
        let mut out = Vec::new();
        packet.encode(&mut out).unwrap();
        assert_eq!(out, bytes);
    }
}
