use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::system::{Query, Res};
pub use mcrs_minecraft_damage::{ActiveEffect, EffectAction, Effects};
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::DamageSource;
pub use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRemoveMobEffect;

use crate::components::{Armor, Dying, PlayerEffects, PlayerFood, PlayerVitals};
use crate::damage_rules::DamageTypeRulesResource;
use crate::events::{DamageRequest, DamageTarget};

pub use mcrs_minecraft_damage::ActiveEffect as Effect;
pub use mcrs_minecraft_damage::Effects as ActiveEffects;
pub use mcrs_minecraft_entity::mob_effect_keys::MobEffect as EffectKey;

/// Message triggered when an effect is applied.
#[derive(Message, Clone, Debug)]
pub struct ApplyEffectEvent {
    pub entity: Entity,
    pub effect: ActiveEffect,
}

/// Message triggered when an effect is removed.
#[derive(Message, Clone, Debug)]
pub struct RemoveEffectEvent {
    pub entity: Entity,
    pub effect: MobEffect,
}

/// Applies instant effects and stores the lasting ones (EFX-ADD-001, EFX-ADD-002).
pub fn handle_apply_effect_events(
    mut events: MessageReader<ApplyEffectEvent>,
    mut query: Query<(
        &mut PlayerVitals,
        &mut PlayerEffects,
        Option<&Armor>,
        Option<&Dying>,
    )>,
    rules: Option<Res<DamageTypeRulesResource>>,
    mut damage_events: MessageWriter<DamageRequest>,
) {
    for ev in events.read() {
        let Ok((mut vitals, mut effects, armor, dying)) = query.get_mut(ev.entity) else {
            continue;
        };
        if dying.is_some() {
            continue;
        }
        let amplifier = ev.effect.amplifier;
        match ev.effect.effect {
            MobEffect::InstantHealth => {
                let amount = 4.0 * f32::from(1u16 << amplifier.min(15));
                vitals.0.health = (vitals.0.health + amount).min(vitals.0.max_health);
            }
            MobEffect::InstantDamage => {
                let amount = 6.0 * f32::from(1u16 << amplifier.min(15));
                let id = rules
                    .as_deref()
                    .and_then(|rules| rules.0.id_path("magic"))
                    .unwrap_or(0);
                damage_events.write(DamageRequest::player(
                    ev.entity,
                    Hit::from_source(amount, DamageSource::new(id)),
                ));
            }
            MobEffect::Absorption => {
                effects.0.add(ev.effect);
                vitals.0.absorption = effects.0.absorption_cap();
            }
            MobEffect::HealthBoost => {
                effects.0.add(ev.effect);
                vitals.0.max_health = 20.0 + effects.0.health_boost();
                vitals.0.health = vitals.0.health.min(vitals.0.max_health);
            }
            _ => {
                effects.0.add(ev.effect);
            }
        }
        let _ = armor;
    }
}

/// Advances active status effects by one tick (EFX-TCK-001, EFX-TCK-002).
pub fn tick_active_effects(
    mut query: Query<(
        Entity,
        &mut PlayerVitals,
        &mut PlayerEffects,
        Option<&mut PlayerFood>,
        Option<&Armor>,
        Option<&Dying>,
    )>,
    rules: Option<Res<DamageTypeRulesResource>>,
    mut damage_events: MessageWriter<DamageRequest>,
    mut remove_events: MessageWriter<RemoveEffectEvent>,
) {
    for (entity, mut vitals, mut effects, mut food, _armor, dying) in &mut query {
        if dying.is_some() {
            continue;
        }
        let mut actions = Vec::new();
        let removed = effects.0.tick(
            false,
            vitals.0.health,
            vitals.0.absorption,
            |action| actions.push(action),
        );
        for action in actions {
            match action {
                EffectAction::Heal(amount) => {
                    vitals.0.health = (vitals.0.health + amount).min(vitals.0.max_health);
                }
                EffectAction::Damage { magic, amount } => {
                    let name = if magic { "magic" } else { "wither" };
                    let id = rules
                        .as_deref()
                        .and_then(|rules| rules.0.id_path(name))
                        .unwrap_or(0);
                    damage_events.write(DamageRequest::player(
                        entity,
                        Hit::from_source(amount, DamageSource::new(id)),
                    ));
                }
                EffectAction::Exhaustion(amount) => {
                    if let Some(ref mut food) = food {
                        food.0.add_exhaustion(amount);
                    }
                }
                EffectAction::Saturation { food: add, saturation } => {
                    if let Some(ref mut food) = food {
                        food.0.food_level = food.0.food_level.saturating_add(add).min(20);
                        food.0.saturation =
                            (food.0.saturation + saturation).min(food.0.food_level as f32);
                    }
                }
            }
        }
        for effect in removed {
            remove_events.write(RemoveEffectEvent { entity, effect });
            match effect {
                MobEffect::Absorption => vitals.0.absorption = 0.0,
                MobEffect::HealthBoost => {
                    vitals.0.max_health = 20.0;
                    vitals.0.health = vitals.0.health.min(vitals.0.max_health);
                }
                _ => {}
            }
        }
    }
}
