use mcrs_minecraft_text::{
    ClickEvent, Color, DataSource, HoverEvent, IntoText, ItemHover, ObjectInfo, Text, TextContent,
};
use mcrs_minecraft_text_bedrock::{
    BedrockServerResolver, BedrockText, DefaultServerResolver, has_translate, to_bedrock,
    to_bedrock_rawtext, to_bedrock_string,
};

struct TestServerResolver;

impl BedrockServerResolver for TestServerResolver {
    fn translation_key<'a>(&'a self, key: &'a str) -> &'a str {
        if key == "mcrs:respawn.blocked" {
            "respawn.blocked"
        } else {
            key
        }
    }

    fn resolve_nbt(&self, nbt: &str, _source: &DataSource) -> Option<String> {
        (nbt == "CustomName").then(|| "§mExcalibur".to_string())
    }

    fn resolve_score(&self, name: &str, objective: &str) -> Option<String> {
        (name == "Player" && objective == "kills").then(|| "42".to_string())
    }

    fn resolve_selector(&self, selector: &str) -> Option<String> {
        (selector == "@p").then(|| "Alex".to_string())
    }

    fn resolve_keybind(&self, keybind: &str) -> Option<String> {
        (keybind == "key.jump").then(|| "SPACE".to_string())
    }
}

fn no_args() -> Vec<Text<ItemHover>> {
    Vec::new()
}

#[test]
fn plain_text_to_bedrock_string() {
    let text: Text<ItemHover> = Text::text("Hello world");
    let (bedrock, drops) = to_bedrock(&text, &DefaultServerResolver);
    assert_eq!(bedrock, BedrockText::String("Hello world".to_string()));
    assert!(drops.is_empty());
}

#[test]
fn string_output() {
    let mut nbt: Text<ItemHover> = Text::text("");
    nbt.content = TextContent::Nbt {
        nbt: "CustomName".into(),
        interpret: false,
        plain: false,
        separator: None,
        source: DataSource::Storage {
            typed: (),
            storage: "minecraft:names".parse().unwrap(),
        },
    };
    let cases: [(Text<ItemHover>, &str); 7] = [
        (
            Text::text("Styled").color(Color::RED).bold().italic(),
            "§c§l§oStyled§r",
        ),
        (
            Text::text("Struck").strikethrough().underlined().bold(),
            "§lStruck§r",
        ),
        (
            Text::text("§aGreen §mStruck §nUnder §cRed"),
            "§aGreen Struck Under §cRed",
        ),
        (nbt, "Excalibur"),
        (Text::selector("@p", None), "Alex"),
        (Text::keybind("key.jump"), "SPACE"),
        (Text::score("Player", "kills"), "42"),
    ];
    for (text, expected) in cases {
        let (s, _) = to_bedrock_string(&text, &TestServerResolver);
        assert_eq!(s, expected);
    }
}

#[test]
fn bedrock_colour_codes_from_java_formatting_are_counted() {
    let text: Text<ItemHover> = Text::text("a§mb").strikethrough().underlined();
    let (_, drops) = to_bedrock_string(&text, &DefaultServerResolver);
    assert_eq!((drops.strikethrough, drops.underlined), (2, 1));
}

#[test]
fn drop_click_hover_and_font() {
    let mut text: Text<ItemHover> = Text::text("Interactive");
    text.style.click_event = Some(ClickEvent::RunCommand {
        command: "/help".into(),
    });
    text.style.hover_event = Some(HoverEvent::ShowText {
        value: Text::text("Help hover"),
    });
    text.style.font = Some("minecraft:alt".parse().unwrap());

    let (_, drops) = to_bedrock(&text, &DefaultServerResolver);
    assert_eq!(
        (drops.click_events, drops.hover_events, drops.fonts),
        (1, 1, 1)
    );
}

#[test]
fn string_translation_fills_its_fallback() {
    let mut text: Text<ItemHover> =
        Text::translate_args("mcrs:greeting", vec![Text::text("Bob").into()]);
    if let TextContent::Translate { fallback, .. } = &mut text.content {
        *fallback = Some("Hello %s!".into());
    }
    let (s, _) = to_bedrock_string(&text, &DefaultServerResolver);
    assert_eq!(s, "Hello Bob!");
}

#[test]
fn rawtext_output() {
    let nested = Text::translate_args(
        "chat.type.announcement",
        vec![
            Text::text("Server").into(),
            Text::translate("item.apple.name", no_args())
                .add_child(" pie")
                .into(),
        ],
    );
    let cases: [(Text<ItemHover>, &str); 5] = [
        (
            Text::translate("mcrs:respawn.blocked", no_args()),
            r#"{"rawtext":[{"translate":"respawn.blocked"}]}"#,
        ),
        (
            Text::translate("chat.type.announcement", vec![Text::text("Server")])
                .color(Color::YELLOW)
                .bold(),
            r#"{"rawtext":[{"text":"§e§l"},{"translate":"chat.type.announcement","with":["Server"]},{"text":"§r"}]}"#,
        ),
        (
            Text::translate_args(
                "chat.type.text",
                vec![
                    Text::text("Steve").color(Color::GREEN).into(),
                    Text::text("Hello world!").into(),
                ],
            ),
            r#"{"rawtext":[{"translate":"chat.type.text","with":["§aSteve§r","Hello world!"]}]}"#,
        ),
        (
            Text::translate_args(
                "chat.type.text",
                vec![Text::text("Steve").color(Color::GREEN).into()],
            )
            .color(Color::YELLOW),
            r#"{"rawtext":[{"text":"§e"},{"translate":"chat.type.text","with":["§aSteve§e"]},{"text":"§r"}]}"#,
        ),
        (
            nested,
            r#"{"rawtext":[{"translate":"chat.type.announcement","with":{"rawtext":[{"text":"Server"},{"rawtext":[{"translate":"item.apple.name"},{"text":" pie"}]}]}}]}"#,
        ),
    ];
    for (text, expected) in cases {
        let (rawtext, _) = to_bedrock_rawtext(&text, &TestServerResolver);
        assert_eq!(serde_json::to_string(&rawtext).unwrap(), expected);
    }
}

#[test]
fn rawtext_serialization_round_trip() {
    let text: Text<ItemHover> = Text::translate_args(
        "chat.type.announcement",
        vec![
            Text::text("Server").into(),
            Text::translate("gui.done", no_args()).add_child("!").into(),
        ],
    );
    let (rawtext, _) = to_bedrock_rawtext(&text, &DefaultServerResolver);
    let decoded: mcrs_minecraft_text_bedrock::BedrockRawtext =
        serde_json::from_str(&serde_json::to_string(&rawtext).unwrap()).unwrap();
    assert_eq!(rawtext, decoded);
}

#[test]
fn oversized_text_is_truncated_and_reported() {
    let text: Text<ItemHover> = Text::text("x".repeat(300_000));
    let (s, drops) = to_bedrock_string(&text, &DefaultServerResolver);
    assert!(s.len() < 300_000);
    assert!(drops.truncated);
}

#[test]
fn has_translate_detects_fallback_in_object() {
    let mut text: Text<ItemHover> = Text::text("Parent");
    text.content = TextContent::Object {
        object: ObjectInfo::Atlas {
            typed: (),
            atlas: "minecraft:default".parse().unwrap(),
            sprite: "minecraft:icon".parse().unwrap(),
        },
        fallback: Some(Text::translate("gui.done", no_args())),
    };
    assert!(has_translate(&text));
}
