use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::*;
use mcrs_minecraft_damage::{DamageTypeFlags, Defence, HitOutcome, hurt};
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine_world::entity::physics::Velocity;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_rule::GameRule;

use crate::components::{
    Armor, Dying, KnockbackResistance, PlayerEffects, PlayerFood, PlayerHitRecord, PlayerVitals,
};
use crate::damage_rules::DamageTypeRulesResource;
use crate::events::{
    DamageDealt, DamageRequest, DamageTarget, DeathEvent, KnockbackRequest,
};

/// Applies every queued player hit through the one damage core (DMG-HIT-001 to DMG-HIT-005).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system: each component of a damaged player is a separate query term"
)]
pub fn process_damage_requests(
    mut commands: Commands,
    mut requests: MessageReader<DamageRequest>,
    mut damage_dealt: MessageWriter<DamageDealt>,
    mut death_events: MessageWriter<DeathEvent>,
    mut knockbacks: MessageWriter<KnockbackRequest>,
    rules: Option<Res<DamageTypeRulesResource>>,
    game_rules: Option<Res<GameRules>>,
    time: Res<GameTime>,
    mut query: Query<(
        Entity,
        &mut PlayerVitals,
        &mut PlayerHitRecord,
        Option<&mut PlayerFood>,
        Option<&PlayerEffects>,
        Option<&Armor>,
        Option<&KnockbackResistance>,
        Option<&PlayerGameMode>,
        Option<&Dying>,
    )>,
) {
    for req in requests.read() {
        let DamageTarget::Player(entity) = req.target else {
            continue;
        };
        let Ok((
            _,
            mut vitals,
            mut record,
            mut food,
            effects,
            armor,
            resistance,
            mode,
            dying,
        )) = query.get_mut(entity)
        else {
            continue;
        };
        if dying.is_some() {
            continue;
        }
        let flags = rules
            .as_deref()
            .map_or(DamageTypeFlags::default(), |rules| {
                rules.0.flags(req.hit.damage_type)
            });
        if let Some(mode) = mode
            && matches!(mode.0, GameMode::Creative | GameMode::Spectator)
            && !flags.bypasses_invulnerability
        {
            continue;
        }
        if let Some(game_rules) = game_rules.as_deref() {
            if flags.is_fall && !game_rules.is_on(GameRule::FallDamage)
                || flags.is_fire && !game_rules.is_on(GameRule::FireDamage)
                || flags.is_drowning && !game_rules.is_on(GameRule::DrowningDamage)
                || flags.is_freezing && !game_rules.is_on(GameRule::FreezeDamage)
            {
                continue;
            }
        }
        let fire_resistance = effects.is_some_and(|effects| {
            effects.0.has(MobEffect::FireResistance) && !flags.bypasses_effects
        });
        let defence = Defence {
            flags,
            armor: armor.map_or(0.0, |armor| armor.armor),
            armor_toughness: armor.map_or(0.0, |armor| armor.toughness),
            resistance: effects.and_then(|effects| effects.0.resistance()),
            max_absorption: effects.map_or(0.0, |effects| effects.0.absorption_cap()),
            fire_resistance,
            fire_immune: false,
            fall_immune: false,
            undead: false,
        };
        let health_before = vitals.0.health;
        let before = vitals.0.health + vitals.0.absorption;
        let outcome = hurt(
            &mut vitals.0,
            &mut record.0,
            &defence,
            req.hit,
            time.0 as u64,
        );
        if outcome == HitOutcome::Rejected {
            continue;
        }
        let after = vitals.0.health + vitals.0.absorption;
        let damage = (before - after).max(0.0);
        if let Some(ref mut food) = food
            && vitals.0.health < health_before
        {
            food.0.add_exhaustion(
                rules
                    .as_deref()
                    .map_or(0.0, |rules| rules.0.exhaustion(req.hit.damage_type)),
            );
        }
        damage_dealt.write(DamageDealt {
            target: req.target,
            hit: req.hit,
            outcome,
            damage,
        });
        if outcome == HitOutcome::Full || outcome == HitOutcome::Partial {
            let strength = req.hit.knockback
                * (1.0 - resistance.map_or(0.0, |resistance| resistance.0 as f32).clamp(0.0, 1.0));
            if strength > 0.0 {
                knockbacks.write(KnockbackRequest {
                    target: entity,
                    direction: bevy_math::Vec3::Z,
                    strength,
                });
            }
        }
        if vitals.0.death_time == Some(0) {
            commands.entity(entity).insert(Dying);
            death_events.write(DeathEvent {
                target: req.target,
                source: req.hit.source,
            });
        }
    }
}

/// Counts the death time of a player and removes the dying marker after twenty ticks
/// (DMG-DTH-001).
pub fn tick_death(
    mut commands: Commands,
    mut query: Query<(Entity, &mut PlayerVitals, &Dying)>,
) {
    for (entity, mut vitals, _) in &mut query {
        let Some(time) = vitals.0.death_time.as_mut() else {
            continue;
        };
        *time = time.saturating_add(1);
        if *time >= 20 {
            commands.entity(entity).remove::<Dying>();
        }
    }
}

/// Lowers the damage cooldown and the hurt time of each player by one tick (DMG-HIT-003).
pub fn tick_player_cooldowns(mut query: Query<&mut PlayerVitals>) {
    for mut vitals in &mut query {
        vitals.0.damage_cooldown = vitals.0.damage_cooldown.saturating_sub(1);
        vitals.0.hurt_time = vitals.0.hurt_time.saturating_sub(1);
    }
}

/// Pushes a player along the horizontal direction of a hit (DMG-KB-001).
pub fn apply_knockback(
    mut requests: MessageReader<KnockbackRequest>,
    mut query: Query<&mut Velocity>,
) {
    for request in requests.read() {
        if let Ok(mut velocity) = query.get_mut(request.target) {
            velocity.0.x += f64::from(request.direction.x) * f64::from(request.strength);
            velocity.0.z += f64::from(request.direction.z) * f64::from(request.strength);
            velocity.0.y = velocity.0.y.max(f64::from(request.strength) * 0.4);
        }
    }
}
