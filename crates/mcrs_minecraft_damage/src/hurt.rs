use mcrs_minecraft_physics::{HitRecord, Vitals};

use crate::Defence;
use crate::Hit;
use crate::HitOutcome;

/// The damage cooldown above which a hit is a partial hit (DMG-HIT-003).
const PARTIAL_ABOVE: u8 = 10;
const COOLDOWN: u8 = 20;
const HURT_TIME: u8 = 10;

/// Does `hit` in tick `tick` on `vitals` and `record` (DMG-HIT-001 to DMG-HIT-005, DMG-DTH-001).
/// A dying entity rejects the hit and records nothing. Each other hit records the strike for the
/// idle time, also one that the cooldown rejects; an accepted hit becomes the last hit of
/// `record`. An accepted hit that takes the health to 0 starts the death.
pub fn hurt(
    vitals: &mut Vitals,
    record: &mut HitRecord,
    defence: &Defence,
    hit: Hit,
    tick: u64,
) -> HitOutcome {
    if vitals.death_time.is_some() {
        return HitOutcome::Rejected;
    }
    if defence.flags.is_fire && (defence.fire_immune || defence.fire_resistance) {
        return HitOutcome::Rejected;
    }
    if defence.flags.is_fall && defence.fall_immune {
        return HitOutcome::Rejected;
    }
    record.strike();
    let amount = normalize(hit.amount);
    let outcome = if vitals.damage_cooldown > PARTIAL_ABOVE && !defence.flags.bypasses_cooldown {
        if amount <= vitals.last_damage {
            return HitOutcome::Rejected;
        }
        apply(vitals, defence, amount - vitals.last_damage);
        HitOutcome::Partial
    } else {
        vitals.damage_cooldown = COOLDOWN;
        vitals.hurt_time = HURT_TIME;
        apply(vitals, defence, amount);
        HitOutcome::Full
    };
    vitals.last_damage = amount;
    if vitals.health <= 0.0 {
        vitals.death_time = Some(0);
    }
    record.accept(tick, hit.source.damage_type);
    outcome
}

/// What `hit` would do to `vitals`, which stays as it is.
#[must_use]
pub fn preview(vitals: &Vitals, defence: &Defence, hit: Hit, tick: u64) -> HitOutcome {
    let mut vitals = *vitals;
    let mut record = HitRecord::default();
    hurt(&mut vitals, &mut record, defence, hit, tick)
}

fn normalize(amount: f32) -> f32 {
    if amount.is_finite() {
        amount.max(0.0)
    } else {
        f32::MAX
    }
}

fn apply(vitals: &mut Vitals, defence: &Defence, amount: f32) {
    let mut damage = amount;
    if !defence.flags.bypasses_armor {
        damage = crate::reduce_by_armor(damage, defence.armor, defence.armor_toughness);
    }
    if !defence.flags.bypasses_effects
        && !defence.flags.bypasses_resistance
        && let Some(amplifier) = defence.resistance
    {
        damage = crate::reduce_by_resistance(damage, amplifier);
    }
    let max_absorption = defence.max_absorption.max(0.0);
    let absorption = vitals.absorption.clamp(0.0, max_absorption);
    let health_damage = (damage - absorption).max(0.0);
    vitals.absorption = (absorption - (damage - health_damage)).clamp(0.0, max_absorption);
    if health_damage != 0.0 {
        vitals.health = (vitals.health - health_damage).clamp(0.0, vitals.max_health);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pig() -> Vitals {
        Vitals {
            health: 10.0,
            max_health: 20.0,
            ..Vitals::default()
        }
    }

    #[test]
    fn hit_003_the_cooldown_rejects_an_equal_hit_and_applies_the_rest_of_a_larger_one() {
        let mut vitals = pig();
        let mut record = HitRecord::default();
        let defence = Defence::default();
        let zero = Hit::new(0.0, 0);
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, zero, 0),
            HitOutcome::Full
        );
        assert_eq!(
            (vitals.health, vitals.hurt_time, vitals.damage_cooldown),
            (10.0, 10, 20)
        );
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, zero, 1),
            HitOutcome::Rejected
        );
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, Hit::new(3.0, 0), 2),
            HitOutcome::Partial
        );
        assert_eq!(vitals.health, 7.0);
        vitals.damage_cooldown = 10;
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, Hit::new(1.0, 0), 3),
            HitOutcome::Full
        );
        assert_eq!(vitals.health, 6.0);
    }

    #[test]
    fn hit_002_a_hit_that_the_cooldown_rejects_still_strikes_the_mob() {
        let mut vitals = pig();
        let mut record = HitRecord::default();
        let defence = Defence::default();
        let drown = Hit::new(0.0, 3);
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, drown, 10),
            HitOutcome::Full
        );
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, drown, 12),
            HitOutcome::Rejected
        );
        assert_eq!(record.strikes(), 2);
        assert_eq!(record.last().map(|hit| hit.tick), Some(10));

        vitals.death_time = Some(1);
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, drown, 40),
            HitOutcome::Rejected
        );
        assert_eq!(record.strikes(), 2);
    }

    #[test]
    fn hit_004_absorption_takes_damage_before_health() {
        let mut vitals = pig();
        vitals.absorption = 4.0;
        let mut record = HitRecord::default();
        let defence = Defence {
            max_absorption: 10.0,
            ..Defence::default()
        };
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, Hit::new(5.0, 0), 0),
            HitOutcome::Full
        );
        assert_eq!(vitals.absorption, 0.0);
        assert_eq!(vitals.health, 9.0);
    }

    #[test]
    fn hit_001_a_fire_hit_does_nothing_to_a_fire_immune_entity() {
        let mut vitals = pig();
        let mut record = HitRecord::default();
        let defence = Defence {
            flags: crate::DamageTypeFlags {
                is_fire: true,
                ..crate::DamageTypeFlags::default()
            },
            fire_immune: true,
            ..Defence::default()
        };
        assert_eq!(
            hurt(&mut vitals, &mut record, &defence, Hit::new(5.0, 0), 0),
            HitOutcome::Rejected
        );
        assert_eq!(vitals.health, 10.0);
    }

    #[test]
    fn dth_004_a_dying_mob_rejects_each_hit() {
        let mut vitals = pig();
        vitals.death_time = Some(3);
        assert_eq!(
            hurt(
                &mut vitals,
                &mut HitRecord::default(),
                &Defence::default(),
                Hit::new(f32::MAX, 0),
                0
            ),
            HitOutcome::Rejected
        );
    }
}
