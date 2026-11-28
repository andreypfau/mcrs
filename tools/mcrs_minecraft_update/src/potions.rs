use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Deserialize;

use crate::keys;

pub const POTION_DEFINITIONS: &str = "assets/mcrs/registry_definition/potion.json";
pub const POTION_ENUM_FILE: &str = "crates/mcrs_minecraft_potion/src/potion_keys.rs";
pub const EFFECT_DEFINITIONS: &str = "assets/mcrs/registry_definition/mob_effect.json";
pub const EFFECT_ENUM_FILE: &str = "crates/mcrs_minecraft_entity/src/mob_effect_keys.rs";
pub const POTION: &str = "minecraft:potion";
pub const MOB_EFFECT: &str = "minecraft:mob_effect";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PotionDefinition {
    effects: Vec<Effect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Effect {
    id: String,
    amplifier: u8,
    duration: i32,
    ambient: bool,
    show_particles: bool,
    show_icon: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectDefinition {
    color: u32,
}

fn variant(registry: &str, name: &str) -> Result<String, String> {
    Ok(keys::variant(&keys::constant(registry, name)?))
}

pub fn potion_effects_impl(json: &str) -> Result<String, String> {
    let definitions: BTreeMap<String, PotionDefinition> =
        serde_json::from_str(json).map_err(|error| format!("{POTION_DEFINITIONS}: {error}"))?;
    let mut out = String::from(
        "\nuse crate::PotionEffect;\n\
         use mcrs_minecraft_entity::mob_effect_keys::MobEffect;\n\n\
         impl Potion {\n\
         #[expect(clippy::too_many_lines, reason = \"one arm per potion the game ships\")]\n\
         pub const fn effects(self) -> &'static [PotionEffect] {\n\
         match self {\n",
    );
    for (name, definition) in &definitions {
        let potion = variant(POTION, name)?;
        let mut effects = String::new();
        for effect in &definition.effects {
            let kind = variant(MOB_EFFECT, &effect.id)?;
            let _ = write!(
                effects,
                "PotionEffect {{\n\
                 effect: MobEffect::{kind},\n\
                 amplifier: {},\n\
                 duration: {},\n\
                 ambient: {},\n\
                 show_particles: {},\n\
                 show_icon: {},\n\
                 }},\n",
                effect.amplifier,
                effect.duration,
                effect.ambient,
                effect.show_particles,
                effect.show_icon,
            );
        }
        let _ = writeln!(out, "Self::{potion} => &[{effects}],");
    }
    out.push_str("}\n}\n}\n");
    Ok(out)
}

pub fn effect_colors_impl(json: &str) -> Result<String, String> {
    let definitions: BTreeMap<String, EffectDefinition> =
        serde_json::from_str(json).map_err(|error| format!("{EFFECT_DEFINITIONS}: {error}"))?;
    let mut out = String::from(
        "\nimpl MobEffect {\n\
         pub const fn color(self) -> u32 {\n\
         match self {\n",
    );
    for (name, definition) in &definitions {
        let effect = variant(MOB_EFFECT, name)?;
        let _ = writeln!(out, "Self::{effect} => {:#08X},", definition.color);
    }
    out.push_str("}\n}\n}\n");
    Ok(out)
}
