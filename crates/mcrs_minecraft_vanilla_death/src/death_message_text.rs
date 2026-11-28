use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_entity::damage_type::DeathMessageType;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;

const BUG_REPORT: &str = "MCPE-28723";
const BUG_REPORT_URL: &str = "https://bugs.mojang.com/browse/MCPE-28723";

#[must_use]
pub fn death_message_text(
    victim: Text,
    damage_type: Option<&DamageType>,
    attacker: Option<Text>,
) -> Text {
    let Some(damage_type) = damage_type else {
        return Text::translate("death.attack.generic", vec![victim]);
    };
    let base = format!("death.attack.{}", damage_type.message_id);
    if damage_type.death_message_type == DeathMessageType::IntentionalGameDesign {
        let link = bug_report_link(&base);
        return Text::translate(format!("{base}.message"), vec![victim, link]);
    }
    match attacker {
        Some(attacker) => Text::translate(base, vec![victim, attacker]),
        None => Text::translate(base, vec![victim]),
    }
}

fn bug_report_link(base: &str) -> Text {
    Text::text("[")
        .add_child(Text::translate(format!("{base}.link"), Vec::new()))
        .add_child(Text::text("]"))
        .on_click_open_url(BUG_REPORT_URL)
        .on_hover_show_text(Text::text(BUG_REPORT))
}
