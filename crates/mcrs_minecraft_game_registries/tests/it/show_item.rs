use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::Text;

use crate::common::read_named;

fn text(id: &str, count: &str, components: &str) -> String {
    format!(
        r#"{{"text": "x", "hover_event": {{
            "action": "show_item", "id": {id}, "count": {count}, "components": {{{components}}}
        }}}}"#
    )
}

#[test]
fn data_pack_text_checks_its_item_hover_at_load() {
    let set = test_registries();
    let example = r#""custom_name": "Wand", "!lore": {}"#;
    let loads = [
        ("REG-FLD-442 example", text(r#""stick""#, "3", example)),
        (
            "REG-FLD-442 unknown hover fields",
            r#"{"text": "x", "hover_event": {"action": "show_item", "id": "stick", "tag": "{}"}}"#
                .to_owned(),
        ),
        (
            "REG-FLD-502 removal fields",
            text(r#""stick""#, "1", r#""!lore": {"ignored": 1}"#),
        ),
    ];
    for (case, json) in &loads {
        if let Err(error) = read_named::<Text>(set, json) {
            panic!("{case}: {error}");
        }
    }
    let fails = [
        text(r#""minecraft:no_such_item""#, "3", example),
        text(r#""minecraft:air""#, "3", example),
        text(r#""stick""#, "0", example),
        text(r#""stick""#, "100", example),
        text(r#""stick""#, "3", r#""minecraft:no_such_component": {}"#),
        text(r#""stick""#, "3", r#""!minecraft:no_such_component": {}"#),
        text(r#""stick""#, "3", r#""creative_slot_lock": {}"#),
        text(r#""stick""#, "3", r#""!lore": true"#),
        text(
            r#""stick""#,
            "3",
            r#""food": {"nutrition": 1, "saturation": 1.0}, "minecraft:food": {"nutrition": 1, "saturation": 1.0}"#,
        ),
        r#"{"text": "x", "hover_event": {"action": "show_item", "contents": "stick"}}"#.to_owned(),
    ];
    for json in &fails {
        assert!(
            read_named::<Text>(set, json).is_err(),
            "REG-FLD-442 failing twin loads: {json}"
        );
    }
}
