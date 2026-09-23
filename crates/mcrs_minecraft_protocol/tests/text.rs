mod common;

use std::io::Cursor;

use mcrs_minecraft_core::rl;
use mcrs_minecraft_nbt::{from_bytes_unnamed, to_bytes_unnamed};

use std::str::FromStr;

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_protocol::text::*;
use mcrs_minecraft_protocol::{Decode, Encode};
use serde::Deserialize;
use uuid::Uuid;

use common::{hex, nbt_tree, sorted};

#[test]
fn text_round_trip() {
    let before = "foo".color(Color::RED).bold()
        + ("bar".obfuscated().color(Color::YELLOW)
            + "baz".underlined().not_bold().italic().color(Color::BLACK));

    let json = format!("{before:#}");

    let after = Text::from_str(&json).unwrap();

    assert_eq!(before, after);
    assert_eq!(before.to_string(), after.to_string());
}

#[test]
fn non_object_data_types() {
    for input in [r#"["foo", true]"#, r#"["foo", 1.9E10]"#, "9999", "false"] {
        let err = serde_json::from_str::<Text>(input).unwrap_err().to_string();
        assert!(err.contains("a text component data type"), "{input}: {err}");
    }
    let txt: Text = serde_json::from_str(r#"["foo", "true"]"#).unwrap();
    assert_eq!(txt, "foo".into_text() + "true");
}

#[test]
fn content_variants_round_trip_through_json() {
    let nbt = |source, expected| {
        (
            Text::nbt(source, "bar", true, Some("baz".into_text())),
            expected,
        )
    };
    for (txt, expected) in [
        (
            Text::translate(
                "chat.type.advancement.task",
                ["arg1".into_text(), "arg2".into_text()],
            ),
            r#"{"translate":"chat.type.advancement.task","with":["arg1","arg2"]}"#,
        ),
        (
            Text::score("foo", "bar"),
            r#"{"score":{"name":"foo","objective":"bar"}}"#,
        ),
        (
            Text::selector("foo", Some(Text::text("bar").color(Color::RED).bold())),
            r#"{"selector":"foo","separator":{"text":"bar","color":"red","bold":true}}"#,
        ),
        (Text::keybind("foo"), r#"{"keybind":"foo"}"#),
        nbt(
            DataSource::Block {
                typed: (),
                block: "foo".into(),
            },
            r#"{"nbt":"bar","interpret":true,"separator":"baz","block":"foo"}"#,
        ),
        nbt(
            DataSource::Entity {
                typed: (),
                entity: "foo".into(),
            },
            r#"{"nbt":"bar","interpret":true,"separator":"baz","entity":"foo"}"#,
        ),
        nbt(
            DataSource::Storage {
                typed: (),
                storage: rl!("foo").into(),
            },
            r#"{"nbt":"bar","interpret":true,"separator":"baz","storage":"minecraft:foo"}"#,
        ),
    ] {
        let serialized = txt.to_string();
        assert_eq!(serialized, expected);
        assert_eq!(Text::from_str(&serialized).unwrap(), txt);
    }
}

#[test]
fn text_to_legacy_lossy() {
    let text: Text = "Heavily formatted green text\n"
        .bold()
        .italic()
        .strikethrough()
        .underlined()
        .obfuscated()
        .color(Color::GREEN)
        + "Lightly formatted red text\n"
            .not_bold()
            .not_strikethrough()
            .not_obfuscated()
            .color(Color::RED)
        + "Not formatted blue text"
            .not_italic()
            .not_underlined()
            .color(Color::BLUE);

    assert_eq!(
        text.to_legacy_lossy(),
        "§a§k§l§m§n§oHeavily formatted green text\n§r§c§n§oLightly formatted red text\n§r§9Not \
         formatted blue text"
    );
}

#[test]
fn plain_text_travels_as_a_bare_nbt_string() {
    let text = Text::text("hello");

    let mut buf = Vec::new();
    text.encode(&mut buf).unwrap();
    assert_eq!(buf[0], mcrs_minecraft_nbt::STRING_ID);

    let mut r: &[u8] = &buf;
    assert_eq!(Text::decode(&mut r).unwrap(), text);
    assert!(r.is_empty());
}

#[test]
fn a_formatted_component_round_trips_through_nbt() {
    let text = Text::translate(
        "chat.type.text",
        [Text::text("sender").color(Color::YELLOW).italic()],
    ) + "body".color(Color::RED).bold().not_underlined()
        + Text::keybind("key.jump");

    let mut buf = Vec::new();
    text.encode(&mut buf).unwrap();
    assert_eq!(buf[0], mcrs_minecraft_nbt::COMPOUND_ID);

    let mut r: &[u8] = &buf;
    let decoded = Text::decode(&mut r).unwrap();
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    assert_eq!(decoded, text);

    let mut again = Vec::new();
    decoded.encode(&mut again).unwrap();
    assert_eq!(again, buf);
}

#[test]
fn a_component_that_is_neither_a_string_nor_a_compound_is_rejected() {
    let mut r: &[u8] = &[mcrs_minecraft_nbt::INT_ID, 0, 0, 0, 1];
    assert!(Text::decode(&mut r).is_err());
}

#[test]
fn a_boolean_inside_a_content_variant_survives_nbt() {
    let storage = DataSource::Storage {
        typed: (),
        storage: rl!("foo").into(),
    };
    let block = DataSource::Block {
        typed: (),
        block: "foo".into(),
    };
    let entity = DataSource::Entity {
        typed: (),
        entity: "@s".into(),
    };
    for text in [
        Text::nbt(storage, "bar", true, Some("sep".into_text())),
        Text::nbt(block, "bar", false, None),
        Text::nbt(entity, "bar", true, None),
    ] {
        let mut buf = Vec::new();
        text.encode(&mut buf).unwrap();
        let mut r: &[u8] = &buf;
        assert_eq!(Text::decode(&mut r).unwrap(), text);
        assert!(r.is_empty());
    }
}

#[derive(Deserialize)]
struct VanillaCase {
    name: String,
    input: serde_json::Value,
    json: serde_json::Value,
    nbt: String,
}

#[test]
fn every_vanilla_fixture_round_trips_through_json_nbt_and_the_wire() {
    let cases: Vec<VanillaCase> =
        serde_json::from_str(include_str!("fixtures/text/vanilla.json")).unwrap();
    assert_eq!(cases.len(), 38);
    for case in cases {
        let name = &case.name;
        let text: Text = serde_json::from_value(case.input.clone())
            .unwrap_or_else(|e| panic!("{name}: input {e}"));
        let from_output: Text = serde_json::from_value(case.json.clone())
            .unwrap_or_else(|e| panic!("{name}: vanilla json {e}"));
        assert_eq!(text, from_output, "{name}: input and vanilla output differ");
        assert_eq!(
            serde_json::to_value(&text).unwrap(),
            case.json,
            "{name}: json"
        );

        let vanilla_nbt = hex(&case.nbt);
        let mut ours = Vec::new();
        to_bytes_unnamed(&text, &mut ours).unwrap();
        assert_eq!(
            nbt_tree(&ours),
            nbt_tree(&vanilla_nbt),
            "{name}: nbt encode"
        );
        if vanilla_nbt[0] == mcrs_minecraft_nbt::STRING_ID {
            assert_eq!(ours, vanilla_nbt, "{name}: bare string bytes");
        }
        let from_nbt: Text = from_bytes_unnamed(&mut Cursor::new(&vanilla_nbt[..]))
            .unwrap_or_else(|e| panic!("{name}: vanilla nbt {e}"));
        assert_eq!(
            serde_json::to_value(&from_nbt).unwrap(),
            case.json,
            "{name}: nbt decode"
        );

        let mut wire = Vec::new();
        text.encode(&mut wire).unwrap();
        assert_eq!(wire, ours, "{name}: wire is the nbt form");
        let mut r: &[u8] = &vanilla_nbt;
        assert_eq!(
            serde_json::to_value(Text::decode(&mut r).unwrap()).unwrap(),
            case.json,
            "{name}: wire decode"
        );
        assert!(r.is_empty(), "{name}: trailing bytes");
        let mut r: &[u8] = &ours;
        assert_eq!(
            Text::decode(&mut r).unwrap(),
            text,
            "{name}: own wire round trip"
        );
    }
}

#[test]
fn vanilla_rejections() {
    for (input, message) in [
        (
            r#"{"text":"x","click_event":{"action":"open_file","path":"/etc/passwd"}}"#,
            "unknown variant `open_file`",
        ),
        (
            r#"{"text":"x","click_event":{"action":"change_page","page":0}}"#,
            "Value must be positive: 0",
        ),
        ("[]", "List must have contents"),
        (r#"{"bold":true}"#, "did not match any variant"),
    ] {
        let err = Text::from_str(input).unwrap_err().to_string();
        assert!(err.contains(message), "{input}: {err}");
    }
    let mut open_file = Text::text("x");
    open_file.click_event = Some(ClickEvent::OpenFile { path: "/x".into() });
    assert!(serde_json::to_string(&open_file).is_err());
}

#[derive(Deserialize)]
struct ProbeCase {
    name: String,
    input: serde_json::Value,
    json: Option<serde_json::Value>,
    nbt: Option<String>,
    error: Option<String>,
}

/// Where the reader knowingly differs from vanilla: registry membership and
/// selector, NBT-path and coordinate syntax are not checked at parse; a
/// number stands for a boolean in JSON as it does in NBT; `show_item`
/// rejects unknown keys as every component codec here does.
const PROBE_DIVERGENCES: &[&str] = &[
    // `object: player` waits for the profile component.
    "object_player",
    "object_player_nohat",
    "object_player_bare_name",
    "object_player_id",
    "bold_int",
    "nbt_interpret_int",
    "hover_show_item_extra_key",
    "click_show_dialog_unknown",
    "hover_show_item_unknown",
    "hover_show_entity_unknown",
    "selector_bad_syntax",
    "nbt_bad_path",
    "nbt_block_bad",
];

#[test]
fn vanilla_probe_cases_read_and_reject_alike() {
    let cases: Vec<ProbeCase> =
        serde_json::from_str(include_str!("fixtures/text/probe.json")).unwrap();
    assert_eq!(cases.len(), 97);
    let mut failures = Vec::new();
    for case in cases {
        if PROBE_DIVERGENCES.contains(&case.name.as_str()) {
            continue;
        }
        let name = &case.name;
        let parsed = serde_json::from_value::<Text>(case.input.clone());
        match (&case.error, parsed) {
            (Some(_), Ok(text)) => failures.push(format!("{name}: accepted {text}")),
            (Some(_), Err(_)) => {}
            (None, Err(e)) => failures.push(format!("{name}: rejected: {e}")),
            (None, Ok(text)) => {
                let json = serde_json::to_value(&text).unwrap();
                if json != *case.json.as_ref().unwrap() {
                    failures.push(format!("{name}: json {json}"));
                }
                let mut ours = Vec::new();
                to_bytes_unnamed(&text, &mut ours).unwrap();
                let vanilla_nbt = hex(case.nbt.as_ref().unwrap());
                if nbt_tree(&ours) != nbt_tree(&vanilla_nbt) {
                    failures.push(format!("{name}: nbt {}", nbt_tree(&ours)));
                }
                let mut r: &[u8] = &vanilla_nbt;
                match Text::decode(&mut r) {
                    Ok(decoded) => {
                        let mut again = Vec::new();
                        decoded.encode(&mut again).unwrap();
                        if nbt_tree(&again) != nbt_tree(&vanilla_nbt) {
                            failures.push(format!("{name}: wire decode {decoded}"));
                        }
                    }
                    Err(e) => failures.push(format!("{name}: wire decode error {e}")),
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[derive(Deserialize)]
struct WireCase {
    name: String,
    json: serde_json::Value,
    wire: String,
}

#[test]
fn vanilla_stream_codec_bytes_decode_and_re_encode() {
    let cases: Vec<WireCase> =
        serde_json::from_str(include_str!("fixtures/text/wire.json")).unwrap();
    assert_eq!(cases.len(), 9);
    for case in cases {
        let name = &case.name;
        let bytes = hex(&case.wire);
        let mut r: &[u8] = &bytes;
        let text = Text::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(r.is_empty(), "{name}: trailing bytes");
        let mut ours = Vec::new();
        text.encode(&mut ours).unwrap();
        assert_eq!(nbt_tree(&ours), nbt_tree(&bytes), "{name}: re-encode");
        // The json is the Java object's form, where an Integer or Boolean
        // argument is not the byte it becomes on the wire; only its shape is
        // checked.
        let from_json: Text = serde_json::from_value(case.json.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&from_json).unwrap(),
            case.json,
            "{name}: json form"
        );
    }
}

#[test]
fn an_int_array_below_a_list_stays_an_int_array() {
    let mut hover = Text::text("x");
    hover.hover_event = Some(HoverEvent::ShowEntity {
        id: ResourceKey::from_location(rl!("pig").into()),
        uuid: Uuid::from_u128(0x00000001_00000002_00000003_00000004),
        name: None,
    });
    let lines = vec![hover.clone()];
    let mut bytes = Vec::new();
    to_bytes_unnamed(&lines, &mut bytes).unwrap();
    let int_array_uuid = hex("0b0004757569640000000400000001000000020000000300000004");
    assert!(
        bytes
            .windows(int_array_uuid.len())
            .any(|window| window == int_array_uuid),
        "{}",
        nbt_tree(&bytes)
    );
    let mut alone = Vec::new();
    to_bytes_unnamed(&hover, &mut alone).unwrap();
    assert!(
        alone
            .windows(int_array_uuid.len())
            .any(|window| window == int_array_uuid)
    );
    assert_eq!(
        from_bytes_unnamed::<Vec<Text>>(&mut Cursor::new(&bytes)).unwrap(),
        lines
    );
}

#[derive(Deserialize)]
struct NbtCase {
    name: String,
    input: String,
    json: Option<serde_json::Value>,
    snbt: Option<String>,
    error: Option<String>,
}

#[test]
fn vanilla_nbt_bytes_decode_to_the_same_json_and_re_encode_to_the_same_tags() {
    let cases: Vec<NbtCase> = serde_json::from_str(include_str!("fixtures/text/nbt.json")).unwrap();
    assert_eq!(cases.len(), 55);
    let mut failures = Vec::new();
    for case in cases {
        let name = &case.name;
        let bytes = hex(&case.input);
        let mut r: &[u8] = &bytes;
        match (&case.error, Text::decode(&mut r)) {
            (Some(_), Ok(text)) => failures.push(format!("{name}: accepted {text}")),
            (Some(_), Err(_)) => {}
            (None, Err(e)) => failures.push(format!("{name}: rejected: {e}")),
            (None, Ok(text)) => {
                if !r.is_empty() {
                    failures.push(format!("{name}: trailing bytes"));
                }
                let json = serde_json::to_value(&text).unwrap();
                if json != *case.json.as_ref().unwrap() {
                    failures.push(format!("{name}: json {json}"));
                }
                let mut ours = Vec::new();
                text.encode(&mut ours).unwrap();
                let vanilla = sorted(
                    mcrs_minecraft_nbt::snbt::parse_tag(case.snbt.as_ref().unwrap()).unwrap(),
                );
                if nbt_tree(&ours) != vanilla {
                    failures.push(format!("{name}: nbt {} != {vanilla}", nbt_tree(&ours)));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Values the reference jar produced for these inputs.
#[test]
fn vanilla_json_edge_cases() {
    for (input, json) in [
        (
            r#"{"text":"x","shadow_color":[2.0,0.5,-0.5,1.0]}"#,
            r#"{"text":"x","shadow_color":-98432}"#,
        ),
        (
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"minecraft:pig","uuid":"1-2-3-4-5"}}"#,
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"minecraft:pig","uuid":[1,131075,262144,5]}}"#,
        ),
        (
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"pig","uuid":[1.5,2,3,4]}}"#,
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"minecraft:pig","uuid":[1,2,3,4]}}"#,
        ),
        (
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"pig","uuid":"100000000-2-3-4-5"}}"#,
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"minecraft:pig","uuid":[0,131075,262144,5]}}"#,
        ),
        (
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"pig","uuid":"00000001-0002-0003-0004-00000000000"}}"#,
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"minecraft:pig","uuid":[1,131075,262144,0]}}"#,
        ),
        (
            r#"{"text":"x","click_event":{"action":"change_page","page":1e10}}"#,
            r#"{"text":"x","click_event":{"action":"change_page","page":1410065408}}"#,
        ),
        (
            r#"{"text":"x","click_event":{"action":"show_dialog","dialog":{"type":"minecraft:notice","title":"t"}}}"#,
            r#"{"text":"x","click_event":{"action":"show_dialog","dialog":{"type":"minecraft:notice","title":"t"}}}"#,
        ),
        (
            r#"{"text":"x","font":":alt"}"#,
            r#"{"text":"x","font":"minecraft:alt"}"#,
        ),
    ] {
        let text = Text::from_str(input).unwrap_or_else(|e| panic!("{input}: {e}"));
        assert_eq!(
            serde_json::to_value(&text).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            "{input}"
        );
    }
    for url in [
        "https://example.com/a?b=[1]#c[d]",
        "https://user@[::1]:8080/p;a/b?q",
        "https:///x",
        "https://exämple.com/",
        "http:foo",
    ] {
        let input =
            format!(r#"{{"text":"x","click_event":{{"action":"open_url","url":"{url}"}}}}"#);
        Text::from_str(&input).unwrap_or_else(|e| panic!("{url}: {e}"));
    }
    for (input, message) in [
        (
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"pig","uuid":"-1-2-3-4-5"}}"#,
            "Invalid UUID string: -1-2-3-4-5",
        ),
        (
            r#"{"text":"x","hover_event":{"action":"show_entity","id":"pig","uuid":"1-2-3-4-"}}"#,
            "Invalid UUID 1-2-3-4-",
        ),
        (
            r#"{"text":"x","click_event":{"action":"change_page","page":3000000000.0}}"#,
            "Value must be positive: -1294967296",
        ),
        (
            r#"{"nbt":"a","storage":"s","interpret":true,"plain":true}"#,
            "did not match any variant",
        ),
        (
            r#"{"type":"nbt","nbt":"a","storage":"s","interpret":true,"plain":true}"#,
            "'interpret' and 'plain' flags can't be both on",
        ),
        (
            r#"{"text":"x","font":"MC:alt"}"#,
            "Non [a-z0-9_.-] character in namespace of identifier: MC:alt",
        ),
    ] {
        let err = Text::from_str(input).unwrap_err().to_string();
        assert!(err.contains(message), "{input}: {err}");
    }
    for (url, message) in [
        (
            "https://ex ample.com/",
            "Illegal character in authority at index 10",
        ),
        (
            "https://example.com/ä%zz",
            "Malformed escape pair at index 21",
        ),
        (
            "HTTPS://Example.com/%zz",
            "Malformed escape pair at index 20",
        ),
        (
            "https://example.com/a%",
            "Malformed escape pair at index 21",
        ),
        (
            "https://example.com/a#b#c",
            "Illegal character in fragment at index 23",
        ),
        (
            "https://example.com/a[b]",
            "Illegal character in path at index 21",
        ),
        ("https:", "Expected scheme-specific part at index 6"),
        (
            "https://example.com/a%2",
            "Malformed escape pair at index 21",
        ),
        (
            "ftp://example.com",
            "Unsupported protocol in URI: ftp://example.com",
        ),
        ("example.com", "Missing protocol in URI: example.com"),
        (
            "https://example.com/a b",
            "Illegal character in path at index 21",
        ),
    ] {
        let input =
            format!(r#"{{"text":"x","click_event":{{"action":"open_url","url":"{url}"}}}}"#);
        let err = Text::from_str(&input).unwrap_err().to_string();
        assert!(err.contains(message), "{url}: {err}");
    }
}
