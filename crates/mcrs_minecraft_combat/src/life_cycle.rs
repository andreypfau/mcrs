use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::Res;
use bevy_ecs::system::{Commands, Query};

use mcrs_minecraft_damage::{
    DamageTypeRules, HitOutcome, apply_absorption_and_health, reduce_by_armor, reduce_by_resistance,
};
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_rule::GameRule;

use crate::damage_rules::DamageTypeRulesResource;
use crate::effects::{ActiveEffects, MobEffect};
use crate::events::{DamageDealt, DamageRequest, DeathEvent, KnockbackRequest};
use crate::health::{Armor, ArmorToughness, DamageCooldown, Health};
use crate::hunger::FoodData;
use crate::position::CombatPosition;

/// The death state of an entity that died (DMG-DTH-001, DMG-DTH-003).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeathState {
    pub death_time: u8,
}

impl DeathState {
    pub fn tick(&mut self) -> bool {
        if self.death_time < 20 {
            self.death_time += 1;
        }
        self.death_time >= 20
    }
}

/// The system that processes all pending damage requests for entities in the tick.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "a Bevy system: the damage sources, the reduction attributes and the death events are separate parameters"
)]
pub fn process_damage_requests(
    mut commands: Commands,
    mut requests: MessageReader<DamageRequest>,
    mut damage_dealt: MessageWriter<DamageDealt>,
    mut death_events: MessageWriter<DeathEvent>,
    mut knockbacks: MessageWriter<KnockbackRequest>,
    rules: Option<Res<DamageTypeRulesResource>>,
    game_rules: Option<Res<GameRules>>,
    mut query: Query<(
        Entity,
        &mut Health,
        &mut DamageCooldown,
        Option<&Armor>,
        Option<&ArmorToughness>,
        Option<&ActiveEffects>,
        Option<&CombatPosition>,
        Option<&PlayerGameMode>,
        Option<&mut FoodData>,
    )>,
) {
    for req in requests.read() {
        let Ok((entity, mut health, mut cooldown, armor, toughness, effects, position, mode, food)) =
            query.get_mut(req.target)
        else {
            continue;
        };

        let flags = rules
            .as_deref()
            .map_or(req.hit.flags, |rules| rules.0.flags(req.hit.damage_type));
        let hit = req.hit.with_flags(flags);
        if let Some(mode) = mode
            && matches!(mode.0, GameMode::Creative | GameMode::Spectator)
            && !flags.bypasses_invulnerability
        {
            continue;
        }
        if let Some(game_rules) = game_rules.as_deref() {
            if flags.is_fall && !game_rules.is_on(GameRule::FallDamage) {
                continue;
            }
            if flags.is_fire && !game_rules.is_on(GameRule::FireDamage) {
                continue;
            }
            if flags.is_drowning && !game_rules.is_on(GameRule::DrowningDamage) {
                continue;
            }
            if flags.is_freezing && !game_rules.is_on(GameRule::FreezeDamage) {
                continue;
            }
        }
        if flags.is_fire
            && !flags.bypasses_effects
            && effects.is_some_and(|effects| effects.has(MobEffect::FireResistance))
        {
            continue;
        }
        if !health.is_alive() {
            continue;
        }

        let amount = hit.amount.max(0.0);
        let (outcome, damage_to_apply) = if flags.bypasses_cooldown {
            cooldown.cooldown = 20;
            cooldown.hurt_time = 10;
            cooldown.last_damage = amount;
            (HitOutcome::Full, amount)
        } else if cooldown.cooldown > 10 {
            if amount <= cooldown.last_damage {
                continue;
            }
            let diff = amount - cooldown.last_damage;
            cooldown.last_damage = amount;
            (HitOutcome::Partial, diff)
        } else {
            cooldown.cooldown = 20;
            cooldown.hurt_time = 10;
            cooldown.last_damage = amount;
            (HitOutcome::Full, amount)
        };

        let armor_val = armor.map_or(0.0, |a| a.0);
        let toughness_val = toughness.map_or(0.0, |t| t.0);
        let after_armor = if flags.bypasses_armor {
            damage_to_apply
        } else {
            reduce_by_armor(damage_to_apply, armor_val, toughness_val)
        };
        let reduced = if flags.bypasses_resistance || flags.bypasses_effects {
            after_armor
        } else if let Some(resistance) =
            effects.and_then(|effects| effects.amplifier(MobEffect::Resistance))
        {
            reduce_by_resistance(after_armor, resistance)
        } else {
            after_armor
        };

        let (new_health, new_absorption) =
            apply_absorption_and_health(health.current, health.max, health.absorption, reduced);

        health.current = new_health;
        health.absorption = new_absorption;

        if let Some(mut food) = food {
            let exhaustion = rules.as_deref().map_or_else(
                || DamageTypeRules::builtin_exhaustion(hit.damage_type),
                |rules| rules.0.exhaustion(hit.damage_type),
            );
            food.add_exhaustion(exhaustion);
        }

        damage_dealt.write(DamageDealt {
            target: entity,
            hit,
            outcome,
            damage: reduced,
        });

        if outcome == HitOutcome::Full
            && hit.knockback > 0.0
            && !flags.no_knockback
            && let (Some(position), Some(source)) = (position, hit.source.source_position)
        {
            let away = position.0 - source;
            let direction = away.with_y(0.0);
            if direction.length_squared() > 1.0e-6 {
                knockbacks.write(KnockbackRequest {
                    target: entity,
                    direction: direction.normalize().as_vec3(),
                    strength: hit.knockback,
                });
            }
        }

        if health.current == 0.0 {
            commands.entity(entity).insert(DeathState::default());
            death_events.write(DeathEvent {
                entity,
                damage_type: hit.damage_type,
                source: hit.source,
            });
        }
    }
}

/// Pushes the entities that a hit knocked back, in their own physics velocity.
pub fn apply_knockback(
    mut requests: MessageReader<KnockbackRequest>,
    mut entities: Query<(
        &mut mcrs_minecraft_engine_world::entity::physics::Velocity,
        Option<&crate::health::KnockbackResistance>,
    )>,
) {
    for request in requests.read() {
        let Ok((mut velocity, resistance)) = entities.get_mut(request.target) else {
            continue;
        };
        let reduction = (1.0 - resistance.map_or(0.0, |resistance| resistance.0)).clamp(0.0, 1.0);
        let strength = f64::from(request.strength) * reduction;
        if strength <= 0.0 {
            continue;
        }
        velocity.0.x = velocity.0.x / 2.0 + f64::from(request.direction.x) * strength;
        velocity.0.z = velocity.0.z / 2.0 + f64::from(request.direction.z) * strength;
        velocity.0.y = (velocity.0.y / 2.0 + strength.min(0.4)).min(0.4);
    }
}

/// Ticks down damage cooldowns and hurt times for all entities each tick.
pub fn tick_cooldowns(mut query: Query<&mut DamageCooldown>) {
    for mut cd in &mut query {
        cd.tick();
    }
}

/// Advances death timers of dying entities.
pub fn tick_death(mut query: Query<&mut DeathState>) {
    for mut death in &mut query {
        death.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};
    use mcrs_minecraft_damage::Hit;
    use mcrs_minecraft_engine::ability::PlayerGameMode;
    use mcrs_minecraft_engine_world::game_rules::GameRules;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_game_rule::GameRule;

    use crate::damage_types;
    use crate::effects::{ActiveEffect, ActiveEffects, MobEffect};
    use crate::health::{Armor, ArmorToughness};

    #[test]
    fn full_damage_cycle_with_cooldown_and_death() {
        let mut app = App::new();
        app.add_message::<DamageRequest>();
        app.add_message::<DamageDealt>();
        app.add_message::<DeathEvent>();
        app.add_message::<KnockbackRequest>();
        app.add_systems(Update, process_damage_requests);

        let target = app
            .world_mut()
            .spawn((Health::new(20.0), DamageCooldown::default()))
            .id();

        // Hit 1: 5.0 damage
        app.world_mut()
            .write_message(DamageRequest::new(target, Hit::new(5.0, 0)));
        app.update();

        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 15.0);

        let cd = app.world().get::<DamageCooldown>(target).unwrap();
        assert_eq!(cd.cooldown, 20);
        assert_eq!(cd.hurt_time, 10);

        // Hit 2 in cooldown with lower damage (4.0 <= 5.0) -> rejected
        app.world_mut()
            .write_message(DamageRequest::new(target, Hit::new(4.0, 0)));
        app.update();
        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 15.0);

        // Hit 3 in cooldown with higher damage (8.0 > 5.0) -> partial hit of 3.0
        app.world_mut()
            .write_message(DamageRequest::new(target, Hit::new(8.0, 0)));
        app.update();
        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 12.0);

        // Lethal hit: 20.0 damage -> health drops to 0, DeathState attached
        app.world_mut()
            .write_message(DamageRequest::new(target, Hit::new(20.0, 0)));
        app.update();
        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 0.0);
        assert!(app.world().get::<DeathState>(target).is_some());
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<DamageRequest>();
        app.add_message::<DamageDealt>();
        app.add_message::<DeathEvent>();
        app.add_message::<KnockbackRequest>();
        app.add_systems(Update, process_damage_requests);
        app
    }

    fn hit(app: &mut App, target: bevy_ecs::entity::Entity, hit: Hit) {
        app.world_mut()
            .write_message(DamageRequest::new(target, hit));
        app.update();
    }

    #[test]
    fn dmg_hit_001_out_of_world_damage_bypasses_the_damage_cooldown() {
        let mut app = App::new();
        app.add_message::<DamageRequest>();
        app.add_message::<DamageDealt>();
        app.add_message::<DeathEvent>();
        app.add_message::<KnockbackRequest>();
        app.add_systems(Update, process_damage_requests);
        let target = app
            .world_mut()
            .spawn((
                Health::new(20.0),
                DamageCooldown {
                    cooldown: 20,
                    last_damage: 10.0,
                    ..DamageCooldown::default()
                },
            ))
            .id();
        app.world_mut().write_message(DamageRequest::new(
            target,
            Hit::new(1.0, damage_types::OUT_OF_WORLD),
        ));
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 19.0);
    }

    #[test]
    fn dmg_hit_001_a_creative_player_is_immune_except_to_out_of_world() {
        let mut app = app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(20.0),
                DamageCooldown::default(),
                PlayerGameMode(GameMode::Creative),
            ))
            .id();
        hit(&mut app, target, Hit::new(5.0, damage_types::PLAYER_ATTACK));
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 20.0);

        hit(&mut app, target, Hit::new(4.0, damage_types::OUT_OF_WORLD));
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 16.0);
    }

    #[test]
    fn dmg_hit_001_a_fall_bypasses_armor() {
        let mut app = app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(20.0),
                DamageCooldown::default(),
                Armor(30.0),
                ArmorToughness(20.0),
            ))
            .id();
        hit(&mut app, target, Hit::new(10.0, damage_types::FALL));
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 10.0);
    }

    #[test]
    fn dmg_red_002_resistance_uses_the_float_formula() {
        let mut app = app();
        let mut effects = ActiveEffects::new();
        effects.add(ActiveEffect::new(MobEffect::Resistance, 1, 200));
        let target = app
            .world_mut()
            .spawn((Health::new(20.0), DamageCooldown::default(), effects))
            .id();
        hit(
            &mut app,
            target,
            Hit::new(10.0, damage_types::PLAYER_ATTACK),
        );
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 14.0);
    }

    #[test]
    fn dmg_red_002_bypasses_effects_skips_resistance() {
        let mut app = app();
        let mut effects = ActiveEffects::new();
        effects.add(ActiveEffect::new(MobEffect::Resistance, 4, 200));
        let target = app
            .world_mut()
            .spawn((Health::new(20.0), DamageCooldown::default(), effects))
            .id();
        hit(&mut app, target, Hit::new(4.0, damage_types::OUT_OF_WORLD));
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 16.0);
    }

    #[test]
    fn dmg_hit_001_fire_resistance_rejects_fire_damage() {
        let mut app = app();
        let mut effects = ActiveEffects::new();
        effects.add(ActiveEffect::new(MobEffect::FireResistance, 0, 200));
        let target = app
            .world_mut()
            .spawn((Health::new(20.0), DamageCooldown::default(), effects))
            .id();
        hit(&mut app, target, Hit::new(5.0, damage_types::IN_FIRE));
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 20.0);
    }

    #[test]
    fn dmg_hit_001_damage_exhaustion_comes_from_the_damage_type() {
        let mut app = app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(20.0),
                DamageCooldown::default(),
                FoodData::new(20, 0.0),
            ))
            .id();
        hit(&mut app, target, Hit::new(1.0, damage_types::PLAYER_ATTACK));
        assert_eq!(app.world().get::<FoodData>(target).unwrap().exhaustion, 0.1);
        hit(&mut app, target, Hit::new(1.0, damage_types::FALL));
        assert_eq!(app.world().get::<FoodData>(target).unwrap().exhaustion, 0.1);
    }

    #[test]
    fn dmg_hit_001_a_disabled_fall_damage_rule_rejects_a_fall() {
        let mut app = app();
        let mut rules = GameRules::default();
        rules.set(GameRule::FallDamage, 0);
        app.insert_resource(rules);
        let target = app
            .world_mut()
            .spawn((Health::new(20.0), DamageCooldown::default()))
            .id();
        hit(&mut app, target, Hit::new(10.0, damage_types::FALL));
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 20.0);
    }
}
