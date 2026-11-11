use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::{Commands, Query};

use mcrs_minecraft_damage::{HitOutcome, apply_absorption_and_health, reduce_by_armor};

use crate::events::{DamageDealt, DamageRequest, DeathEvent};
use crate::health::{Armor, ArmorToughness, DamageCooldown, Health};

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
    clippy::type_complexity,
    reason = "Bevy query with optional components for armor and toughness"
)]
pub fn process_damage_requests(
    mut commands: Commands,
    mut requests: MessageReader<DamageRequest>,
    mut damage_dealt: MessageWriter<DamageDealt>,
    mut death_events: MessageWriter<DeathEvent>,
    mut query: Query<(
        Entity,
        &mut Health,
        &mut DamageCooldown,
        Option<&Armor>,
        Option<&ArmorToughness>,
    )>,
) {
    for req in requests.read() {
        let Ok((entity, mut health, mut cooldown, armor, toughness)) = query.get_mut(req.target) else {
            continue;
        };

        if !health.is_alive() {
            continue;
        }

        let amount = req.hit.amount.max(0.0);
        let (outcome, damage_to_apply) = if cooldown.cooldown > 10 {
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
        let reduced = reduce_by_armor(damage_to_apply, armor_val, toughness_val);

        let (new_health, new_absorption) = apply_absorption_and_health(
            health.current,
            health.max,
            health.absorption,
            reduced,
        );

        health.current = new_health;
        health.absorption = new_absorption;

        damage_dealt.write(DamageDealt {
            target: entity,
            hit: req.hit,
            outcome,
            damage: reduced,
        });

        if health.current == 0.0 {
            commands.entity(entity).insert(DeathState::default());
            death_events.write(DeathEvent { entity });
        }
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

    #[test]
    fn full_damage_cycle_with_cooldown_and_death() {
        let mut app = App::new();
        app.add_message::<DamageRequest>();
        app.add_message::<DamageDealt>();
        app.add_message::<DeathEvent>();
        app.add_systems(Update, process_damage_requests);

        let target = app
            .world_mut()
            .spawn((Health::new(20.0), DamageCooldown::default()))
            .id();

        // Hit 1: 5.0 damage
        app.world_mut().write_message(DamageRequest::new(target, Hit::new(5.0, 0)));
        app.update();

        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 15.0);

        let cd = app.world().get::<DamageCooldown>(target).unwrap();
        assert_eq!(cd.cooldown, 20);
        assert_eq!(cd.hurt_time, 10);

        // Hit 2 in cooldown with lower damage (4.0 <= 5.0) -> rejected
        app.world_mut().write_message(DamageRequest::new(target, Hit::new(4.0, 0)));
        app.update();
        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 15.0);

        // Hit 3 in cooldown with higher damage (8.0 > 5.0) -> partial hit of 3.0
        app.world_mut().write_message(DamageRequest::new(target, Hit::new(8.0, 0)));
        app.update();
        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 12.0);

        // Lethal hit: 20.0 damage -> health drops to 0, DeathState attached
        app.world_mut().write_message(DamageRequest::new(target, Hit::new(20.0, 0)));
        app.update();
        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 0.0);
        assert!(app.world().get::<DeathState>(target).is_some());
    }
}
