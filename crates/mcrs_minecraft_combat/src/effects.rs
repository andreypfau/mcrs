use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::system::{Query, Res};
pub use mcrs_minecraft_damage::ActiveEffect;
use mcrs_minecraft_damage::{DamageSource, EffectAction, Hit};
pub use mcrs_minecraft_entity::mob_effect_keys::MobEffect;

use crate::components::{
    Armor, BaseMaxHealth, Dying, PlayerEffects, PlayerEffectsHidden, PlayerFood, PlayerVitals,
};
use crate::damage_rules::DamageTypeRulesResource;
use crate::events::DamageRequest;

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
        Option<&mut PlayerEffectsHidden>,
        Option<&BaseMaxHealth>,
        Option<&Armor>,
        Option<&Dying>,
    )>,
    rules: Res<DamageTypeRulesResource>,
    mut damage_events: MessageWriter<DamageRequest>,
) {
    for ev in events.read() {
        let Ok((mut vitals, mut effects, mut hidden, base, armor, dying)) =
            query.get_mut(ev.entity)
        else {
            continue;
        };
        if dying.is_some() {
            continue;
        }
        let amplifier = ev.effect.amplifier;
        let kind = ev.effect.effect;
        let previous = effects.0.get(kind).copied();
        match kind {
            MobEffect::InstantHealth => {
                let amount = 4.0 * f32::from(1u16 << amplifier.min(15));
                vitals.0.health = (vitals.0.health + amount).min(vitals.0.max_health);
            }
            MobEffect::InstantDamage => {
                let amount = 6.0 * f32::from(1u16 << amplifier.min(15));
                let id = rules.0.id_path("magic").unwrap_or(0);
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
                let previous = effects.0.health_boost();
                effects.0.add(ev.effect);
                let base_health = base.map_or(vitals.0.max_health - previous, |base| base.0);
                vitals.0.max_health = base_health + effects.0.health_boost();
                vitals.0.health = vitals.0.health.min(vitals.0.max_health);
            }
            _ => {
                effects.0.add(ev.effect);
            }
        }
        if !matches!(kind, MobEffect::InstantHealth | MobEffect::InstantDamage)
            && let Some(hidden) = hidden.as_deref_mut()
        {
            crate::hidden::merge_hidden(previous, ev.effect, hidden);
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
        Option<&mut PlayerEffectsHidden>,
        Option<&mut PlayerFood>,
        Option<&BaseMaxHealth>,
        Option<&Armor>,
        Option<&Dying>,
    )>,
    rules: Res<DamageTypeRulesResource>,
    mut damage_events: MessageWriter<DamageRequest>,
    mut remove_events: MessageWriter<RemoveEffectEvent>,
) {
    let mut actions = Vec::new();
    for (entity, mut vitals, mut effects, mut hidden, mut food, base, _armor, dying) in &mut query {
        if dying.is_some() {
            continue;
        }
        actions.clear();
        let previous_boost = effects.0.health_boost();
        if let Some(hidden) = hidden.as_deref_mut() {
            crate::hidden::tick_hidden(hidden);
        }
        let removed = effects
            .0
            .tick(false, vitals.0.health, vitals.0.absorption, |action| {
                actions.push(action)
            });
        for action in actions.drain(..) {
            match action {
                EffectAction::Heal(amount) => {
                    vitals.0.health = (vitals.0.health + amount).min(vitals.0.max_health);
                }
                EffectAction::Damage { magic, amount } => {
                    let name = if magic { "magic" } else { "wither" };
                    let id = rules.0.id_path(name).unwrap_or(0);
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
                EffectAction::Saturation {
                    food: add,
                    saturation,
                } => {
                    if let Some(ref mut food) = food {
                        food.0.food_level = food.0.food_level.saturating_add(add).min(20);
                        food.0.saturation =
                            (food.0.saturation + saturation).min(food.0.food_level as f32);
                    }
                }
            }
        }
        for effect in removed {
            let promoted = hidden.as_deref_mut().and_then(|hidden| {
                crate::hidden::promote_hidden(hidden, effect)
                    .filter(|promoted| promoted.duration > 0)
            });
            if let Some(promoted) = promoted {
                effects.0.insert(promoted);
            } else {
                remove_events.write(RemoveEffectEvent { entity, effect });
            }
            match effect {
                MobEffect::Absorption => {
                    vitals.0.absorption = if promoted.is_some() {
                        effects.0.absorption_cap()
                    } else {
                        0.0
                    };
                }
                MobEffect::HealthBoost => {
                    let base_health =
                        base.map_or(vitals.0.max_health - previous_boost, |base| base.0);
                    vitals.0.max_health = base_health + effects.0.health_boost();
                    vitals.0.health = vitals.0.health.min(vitals.0.max_health);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_app::{App, Update};
    use bevy_ecs::message::Messages;

    use super::*;
    use mcrs_minecraft_physics::Vitals;

    #[test]
    fn a_promoted_effect_updates_its_derived_stats() {
        let mut app = App::new();
        app.add_message::<ApplyEffectEvent>();
        app.add_message::<RemoveEffectEvent>();
        app.add_message::<DamageRequest>();
        app.insert_resource(DamageTypeRulesResource::default());
        app.add_systems(Update, handle_apply_effect_events);
        let entity = app
            .world_mut()
            .spawn((
                PlayerVitals(Vitals {
                    health: 20.0,
                    max_health: 20.0,
                    ..Vitals::default()
                }),
                PlayerEffects::new(),
                PlayerEffectsHidden::default(),
                BaseMaxHealth(20.0),
            ))
            .id();
        let mut messages = app.world_mut().resource_mut::<Messages<ApplyEffectEvent>>();
        messages.write(ApplyEffectEvent {
            entity,
            effect: ActiveEffect::new(MobEffect::HealthBoost, 1, 1),
        });
        messages.write(ApplyEffectEvent {
            entity,
            effect: ActiveEffect::new(MobEffect::HealthBoost, 0, 10),
        });
        messages.write(ApplyEffectEvent {
            entity,
            effect: ActiveEffect::new(MobEffect::Absorption, 1, 1),
        });
        messages.write(ApplyEffectEvent {
            entity,
            effect: ActiveEffect::new(MobEffect::Absorption, 0, 10),
        });
        app.update();
        assert_eq!(
            app.world()
                .get::<PlayerVitals>(entity)
                .unwrap()
                .0
                .max_health,
            28.0
        );
        assert_eq!(
            app.world()
                .get::<PlayerVitals>(entity)
                .unwrap()
                .0
                .absorption,
            8.0
        );

        app.add_systems(Update, tick_active_effects);
        app.update();
        let vitals = &app.world().get::<PlayerVitals>(entity).unwrap().0;
        assert_eq!(vitals.max_health, 24.0);
        assert_eq!(vitals.absorption, 4.0);
    }

    #[test]
    fn a_health_boost_returns_a_mob_to_its_own_maximum() {
        let mut app = App::new();
        app.add_message::<ApplyEffectEvent>();
        app.add_message::<RemoveEffectEvent>();
        app.add_message::<DamageRequest>();
        app.insert_resource(DamageTypeRulesResource::default());
        app.add_systems(Update, handle_apply_effect_events);
        let entity = app
            .world_mut()
            .spawn((
                PlayerVitals(Vitals {
                    health: 10.0,
                    max_health: 10.0,
                    ..Vitals::default()
                }),
                PlayerEffects::new(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Messages<ApplyEffectEvent>>()
            .write(ApplyEffectEvent {
                entity,
                effect: ActiveEffect::new(MobEffect::HealthBoost, 0, 1),
            });
        app.update();
        assert_eq!(
            app.world()
                .get::<PlayerVitals>(entity)
                .unwrap()
                .0
                .max_health,
            14.0
        );

        app.add_systems(Update, tick_active_effects);
        app.update();
        assert_eq!(
            app.world()
                .get::<PlayerVitals>(entity)
                .unwrap()
                .0
                .max_health,
            10.0
        );
    }
}
