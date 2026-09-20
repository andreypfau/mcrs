//! Values produced by the vanilla 26.3-snapshot-10 `Advancement`, `DisplayInfo`
//! and `ClientboundUpdateAdvancementsPacket` codecs.

use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_protocol::advancement::{
    Advancement, AdvancementHolder, AdvancementProgress, AdvancementType, CriterionProgress,
    DisplayInfo, PositionedAdvancement, RawAdvancement,
};
use mcrs_minecraft_protocol::item::{ComponentPatch, DecodeCtx, EncodeCtx, Template};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundUpdateAdvancements;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    SeenAdvancementsAction, ServerboundSeenAdvancements,
};
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Decode, Encode};

use crate::harness::TestLookup;

const GOLDEN: &str = include_str!("../fixtures/particles_golden.txt");

fn golden(key: &str) -> &'static str {
    let prefix = format!("{key} = ");
    GOLDEN
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no golden value {key}"))
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn lookup() -> TestLookup {
    let mut lookup = TestLookup::new();
    lookup.registry_with_ids(
        "item",
        &[
            ("air", 0),
            ("stone", 1),
            ("apple", 1007),
            ("diamond_sword", 1050),
        ],
    );
    lookup
}

fn id(text: &str) -> ResourceLocation {
    ResourceLocation::parse(text).unwrap()
}

fn item(path: &str, count: i32, components: &str) -> Template {
    Template::new(
        ResourceKey::from_location(ResourceLocation::minecraft(path)),
        count,
        serde_json::from_str::<ComponentPatch>(components).unwrap(),
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
        holder: AdvancementHolder {
            id: id(name),
            value,
        },
        x,
        y,
    }
}

#[test]
fn an_advancement_matches_the_vanilla_stream_codec() {
    let bytes = hex(golden("root.wire"));
    let mut r = &bytes[..];
    let decoded = Advancement::decode_ctx(&lookup(), &mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(decoded, root());
    let mut out = Vec::new();
    decoded.encode_ctx(&lookup(), &mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));
}

#[test]
fn display_info_matches_the_vanilla_codec() {
    let parsed: DisplayInfo = serde_json::from_str(golden("root_display.json")).unwrap();
    let mut expected = root().display.unwrap();
    expected.announce_to_chat = true;
    assert_eq!(parsed, expected);
    let back: serde_json::Value = serde_json::to_value(&parsed).unwrap();
    let vanilla: serde_json::Value = serde_json::from_str(golden("root_display.json")).unwrap();
    assert_eq!(back, vanilla);

    let parsed: DisplayInfo = serde_json::from_str(golden("child_display.json")).unwrap();
    assert_eq!(parsed, child().display.unwrap());
    let back: serde_json::Value = serde_json::to_value(&parsed).unwrap();
    let vanilla: serde_json::Value = serde_json::from_str(golden("child_display.json")).unwrap();
    assert_eq!(back, vanilla);

    let defaults: DisplayInfo =
        serde_json::from_str(r#"{"icon":"minecraft:apple","title":"t","description":"d"}"#)
            .unwrap();
    assert_eq!(defaults.frame, AdvancementType::Task);
    assert!(defaults.show_toast && defaults.announce_to_chat && !defaults.hidden);
    assert_eq!(
        serde_json::to_string(&defaults).unwrap(),
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
        .map(|raw| raw.resolve(&lookup()).unwrap())
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
            .map(|advancement| RawAdvancement::from_positioned(advancement, &lookup()).unwrap())
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
