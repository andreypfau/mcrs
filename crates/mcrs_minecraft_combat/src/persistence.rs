use mcrs_minecraft_damage::{ActiveEffect, Effects};
use mcrs_minecraft_engine::player_state::{
    Attribute as SavedAttribute, Effect as SavedEffect, PlayerFileState,
};
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_physics::Vitals;

use crate::components::{BaseMaxHealth, PlayerEffects, PlayerExperience, PlayerFood, PlayerVitals};
use crate::runtime::EnvironmentTimers;

const MAX_HEALTH_ATTRIBUTE: &str = "minecraft:max_health";

fn assign<T: PartialEq>(target: &mut T, value: T) {
    if *target != value {
        *target = value;
    }
}

fn saved_effect(active: &ActiveEffect) -> SavedEffect {
    SavedEffect {
        id: active.effect.as_static_str().to_owned(),
        amplifier: active.amplifier,
        duration: if active.duration == u32::MAX {
            -1
        } else {
            i32::try_from(active.duration).unwrap_or(i32::MAX)
        },
        ambient: active.ambient,
        show_particles: active.show_particles,
        show_icon: Some(active.show_icon),
        hidden_effect: None,
    }
}

fn load_effect(saved: &SavedEffect) -> Option<ActiveEffect> {
    let effect = MobEffect::read(&saved.id)?;
    let duration = if saved.duration < 0 {
        u32::MAX
    } else {
        u32::try_from(saved.duration).ok()?
    };
    if duration == 0 {
        return None;
    }
    Some(ActiveEffect {
        effect,
        amplifier: saved.amplifier,
        duration,
        ambient: saved.ambient,
        show_particles: saved.show_particles,
        show_icon: saved.show_icon.unwrap_or(saved.show_particles),
    })
}

fn saved_effect_matches(active: &ActiveEffect, saved: &SavedEffect) -> bool {
    saved.id == active.effect.as_static_str()
}

fn update_saved_effect(saved: &mut SavedEffect, active: &ActiveEffect) {
    assign(
        &mut saved.duration,
        if active.duration == u32::MAX {
            -1
        } else {
            i32::try_from(active.duration).unwrap_or(i32::MAX)
        },
    );
    assign(&mut saved.amplifier, active.amplifier);
    assign(&mut saved.ambient, active.ambient);
    assign(&mut saved.show_particles, active.show_particles);
    assign(&mut saved.show_icon, Some(active.show_icon));
}

fn store_effects(file: &mut PlayerFileState, effects: &PlayerEffects) {
    let same_order = file.active_effects.len() == effects.len()
        && file
            .active_effects
            .iter()
            .zip(effects.iter())
            .all(|(saved, active)| saved_effect_matches(active, saved));
    if !same_order {
        file.active_effects = effects.iter().map(saved_effect).collect();
        return;
    }
    for (saved, active) in file.active_effects.iter_mut().zip(effects.iter()) {
        update_saved_effect(saved, active);
    }
}

fn load_effects(file: &PlayerFileState) -> Effects {
    let mut effects = Effects::new();
    for saved in &file.active_effects {
        if let Some(active) = load_effect(saved) {
            effects.add(active);
        }
    }
    effects
}

fn max_health_base(file: &PlayerFileState) -> f32 {
    file.attributes
        .iter()
        .find(|attribute| attribute.id == MAX_HEALTH_ATTRIBUTE || attribute.id == "max_health")
        .map_or(20.0, |attribute| attribute.base as f32)
        .max(0.0)
}

pub(crate) fn load(
    file: &PlayerFileState,
) -> (
    PlayerVitals,
    BaseMaxHealth,
    PlayerFood,
    PlayerExperience,
    PlayerEffects,
) {
    let effects = load_effects(file);
    let base = max_health_base(file);
    let max_health = base + effects.health_boost();
    let health = Vitals {
        health: file.health.clamp(0.0, max_health),
        max_health,
        absorption: file.absorption_amount.max(0.0),
        last_damage: 0.0,
        damage_cooldown: 0,
        hurt_time: u8::try_from(file.hurt_time).unwrap_or(0),
        death_time: (file.death_time > 0).then(|| u8::try_from(file.death_time).unwrap_or(u8::MAX)),
        fire_ticks: i32::from(file.fire).max(0),
        air: i32::from(file.air),
        frozen_ticks: file.ticks_frozen.max(0),
    };
    let food = PlayerFood(mcrs_minecraft_damage::Food {
        food_level: u32::try_from(file.food_level).unwrap_or(0).min(20),
        saturation: file.food_saturation_level.max(0.0),
        exhaustion: file.food_exhaustion_level.max(0.0),
        tick_timer: u32::try_from(file.food_tick_timer).unwrap_or(0),
    });
    let experience = PlayerExperience(mcrs_minecraft_damage::Experience {
        level: file.xp_level.max(0),
        progress: file.xp_progress.clamp(0.0, 1.0),
        total: file.xp_total.max(0),
        score: file.score,
    });
    (
        PlayerVitals(health),
        BaseMaxHealth(base),
        food,
        experience,
        PlayerEffects(effects),
    )
}

pub(crate) fn store(
    file: &mut PlayerFileState,
    health: &PlayerVitals,
    base: &BaseMaxHealth,
    food: &PlayerFood,
    experience: &PlayerExperience,
    effects: &PlayerEffects,
    timers: &EnvironmentTimers,
) {
    let vitals = &health.0;
    assign(&mut file.health, vitals.health);
    assign(&mut file.absorption_amount, vitals.absorption);
    assign(&mut file.hurt_time, i16::from(vitals.hurt_time));
    assign(
        &mut file.death_time,
        i16::from(vitals.death_time.unwrap_or(0)),
    );
    assign(
        &mut file.fire,
        i16::try_from(vitals.fire_ticks.max(0)).unwrap_or(i16::MAX),
    );
    assign(
        &mut file.air,
        i16::try_from(vitals.air.max(0)).unwrap_or(i16::MAX),
    );
    assign(&mut file.ticks_frozen, vitals.frozen_ticks);
    assign(&mut file.fall_distance, timers.fall_distance);
    assign(&mut file.on_ground, timers.on_ground);
    assign(
        &mut file.food_level,
        i32::try_from(food.food_level).unwrap_or(0),
    );
    assign(
        &mut file.food_tick_timer,
        i32::try_from(food.tick_timer).unwrap_or(i32::MAX),
    );
    assign(&mut file.food_saturation_level, food.saturation);
    assign(&mut file.food_exhaustion_level, food.exhaustion);
    assign(&mut file.xp_level, experience.level);
    assign(&mut file.xp_total, experience.total);
    assign(&mut file.xp_progress, experience.progress);
    assign(&mut file.score, experience.score);

    store_effects(file, effects);

    let base_health = f64::from(base.0);
    if let Some(attribute) = file
        .attributes
        .iter_mut()
        .find(|attribute| attribute.id == MAX_HEALTH_ATTRIBUTE || attribute.id == "max_health")
    {
        assign(&mut attribute.base, base_health);
    } else {
        file.attributes.push(SavedAttribute {
            id: MAX_HEALTH_ATTRIBUTE.to_owned(),
            base: base_health,
            modifiers: Vec::new(),
        });
    }
}
