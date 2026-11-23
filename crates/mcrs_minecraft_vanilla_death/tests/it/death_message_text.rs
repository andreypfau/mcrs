use std::borrow::Cow;

use mcrs_minecraft_entity::damage_type::{
    DamageEffects, DamageScaling, DamageType, DeathMessageType,
};
use mcrs_minecraft_text::{ClickEvent, HoverEvent, Text};
use mcrs_minecraft_vanilla_death::death_message_text;

struct Case {
    name: &'static str,
    damage_type: Option<(&'static str, DeathMessageType)>,
    attacker: Option<&'static str>,
    expected: Text,
}

fn damage_type(message_id: &str, death_message_type: DeathMessageType) -> DamageType {
    DamageType {
        message_id: message_id.to_owned(),
        scaling: DamageScaling::Never,
        exhaustion: 0.0,
        effects: DamageEffects::Hurt,
        death_message_type,
    }
}

fn victim() -> Text {
    Text::text("victim")
}

fn bug_report_link() -> Text {
    let mut link = Text::text("[");
    link.extra.push(Text::translate(
        "death.attack.badRespawnPoint.link",
        Vec::new(),
    ));
    link.extra.push(Text::text("]"));
    link.style.click_event = Some(ClickEvent::OpenUrl {
        url: Cow::Borrowed("https://bugs.mojang.com/browse/MCPE-28723"),
    });
    link.style.hover_event = Some(HoverEvent::ShowText {
        value: Text::text("MCPE-28723"),
    });
    link
}

#[test]
fn the_death_message_follows_the_damage_type_and_the_resolved_attacker() {
    let cases = [
        Case {
            name: "an unknown damage type takes the generic key",
            damage_type: None,
            attacker: Some("zombie"),
            expected: Text::translate("death.attack.generic", vec![victim()]),
        },
        Case {
            name: "PLD-MSG-002 step 1: no attacker",
            damage_type: Some(("inWall", DeathMessageType::Default)),
            attacker: None,
            expected: Text::translate("death.attack.inWall", vec![victim()]),
        },
        Case {
            name: "PLD-MSG-002 step 2: the attacker",
            damage_type: Some(("arrow", DeathMessageType::Default)),
            attacker: Some("skeleton"),
            expected: Text::translate("death.attack.arrow", vec![victim(), Text::text("skeleton")]),
        },
        Case {
            name: "PLD-MSG-001 step 2: intentional_game_design",
            damage_type: Some(("badRespawnPoint", DeathMessageType::IntentionalGameDesign)),
            attacker: Some("bed"),
            expected: Text::translate(
                "death.attack.badRespawnPoint.message",
                vec![victim(), bug_report_link()],
            ),
        },
    ];
    for case in cases {
        let damage_type = case
            .damage_type
            .map(|(message_id, kind)| damage_type(message_id, kind));
        assert_eq!(
            death_message_text(
                victim(),
                damage_type.as_ref(),
                case.attacker.map(Text::text),
            ),
            case.expected,
            "{}",
            case.name
        );
    }
}
