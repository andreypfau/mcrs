use crate::DamageTags;
use crate::DamageTypeRules;
use crate::Defence;
use crate::Hit;
use crate::HitOutcome;
use crate::HitRecord;
use crate::HitReport;
use crate::HurtRules;
use crate::Vitals;

const PARTIAL_ABOVE: u8 = 10;
const COOLDOWN: u8 = 20;
const HURT_TIME: u8 = 10;
const ARMOR_FLOOR_SHARE: f32 = 0.2;
const ARMOR_LIMIT: f32 = 20.0;
const REDUCTION_SCALE: f32 = 25.0;
const RESISTANCE_LEVELS: i32 = 25;
const RESISTANCE_STEP: i32 = 5;

pub fn hurt(
    vitals: &mut Vitals,
    record: &mut HitRecord,
    defence: &Defence,
    types: &DamageTypeRules,
    rules: &HurtRules,
    hit: Hit,
    tick: u64,
) -> HitReport {
    let HurtRules {} = rules;
    let damage_type = hit.source.damage_type;
    let tags = types.tags(damage_type);
    let invulnerable = !tags.contains(DamageTags::BYPASSES_INVULNERABILITY)
        && (defence.ability_invulnerable || defence.invulnerable && !hit.source.by_creative_player);
    if !vitals.is_alive() || invulnerable || defence.immune.intersects(tags) {
        return HitReport::REJECTED;
    }
    record.strike();
    let report = cool_down(vitals, defence, tags, clean(hit.amount));
    if report.outcome != HitOutcome::Rejected {
        record.accept(tick, damage_type);
    }
    report
}

#[must_use]
pub fn preview(
    vitals: &Vitals,
    defence: &Defence,
    types: &DamageTypeRules,
    rules: &HurtRules,
    hit: Hit,
) -> HitReport {
    let mut vitals = *vitals;
    hurt(
        &mut vitals,
        &mut HitRecord::default(),
        defence,
        types,
        rules,
        hit,
        0,
    )
}

fn clean(amount: f32) -> f32 {
    let amount = if amount < 0.0 { 0.0 } else { amount };
    if amount.is_finite() { amount } else { f32::MAX }
}

fn cool_down(vitals: &mut Vitals, defence: &Defence, tags: DamageTags, amount: f32) -> HitReport {
    let cooling =
        vitals.damage_cooldown > PARTIAL_ABOVE && !tags.contains(DamageTags::BYPASSES_COOLDOWN);
    let (outcome, damage) = if cooling {
        if amount <= vitals.last_damage {
            return HitReport::REJECTED;
        }
        (HitOutcome::Partial, amount - vitals.last_damage)
    } else {
        vitals.damage_cooldown = COOLDOWN;
        vitals.hurt_time = HURT_TIME;
        (HitOutcome::Full, amount)
    };
    vitals.last_damage = amount;
    let mut report = apply(vitals, defence, tags, damage);
    report.outcome = outcome;
    if vitals.health <= 0.0 {
        vitals.death_time = Some(0);
        report.died = true;
    }
    report
}

fn apply(vitals: &mut Vitals, defence: &Defence, tags: DamageTags, damage: f32) -> HitReport {
    let applied = reduce(damage, defence, tags);
    let health = (applied - vitals.absorption).max(0.0);
    let absorbed = applied - health;
    vitals.absorption -= absorbed;
    let lost = health.min(vitals.health);
    if health != 0.0 {
        vitals.health = (vitals.health - health).max(0.0);
    }
    HitReport {
        outcome: HitOutcome::Full,
        armor: damage - applied,
        absorption: absorbed,
        health: lost,
        died: false,
    }
}

fn reduce(damage: f32, defence: &Defence, tags: DamageTags) -> f32 {
    let mut damage = damage;
    if !tags.contains(DamageTags::BYPASSES_ARMOR) {
        damage = armored(damage, defence);
    }
    let bypass = DamageTags::BYPASSES_EFFECTS.union(DamageTags::BYPASSES_RESISTANCE);
    if let Some(amplifier) = defence.resistance
        && !tags.intersects(bypass)
    {
        let share = RESISTANCE_LEVELS - (i32::from(amplifier) + 1) * RESISTANCE_STEP;
        damage = (damage * share as f32 / REDUCTION_SCALE).max(0.0);
    }
    damage
}

fn armored(damage: f32, defence: &Defence) -> f32 {
    let armor = libm::floorf(defence.armor);
    let toughness = 2.0 + defence.toughness / 4.0;
    let floor = armor * ARMOR_FLOOR_SHARE;
    let raw = armor - damage / toughness;
    let reduction = if raw < floor {
        floor
    } else {
        raw.min(ARMOR_LIMIT)
    };
    damage * (1.0 - reduction / REDUCTION_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DamageSource;

    const GENERIC: u32 = 0;
    const FALL: u32 = 1;
    const MAGIC: u32 = 2;
    const IN_FIRE: u32 = 3;
    const ALWAYS: u32 = 4;
    const KILL: u32 = 5;

    fn types() -> DamageTypeRules {
        DamageTypeRules::new([
            DamageTags::empty(),
            DamageTags::BYPASSES_ARMOR.union(DamageTags::IS_FALL),
            DamageTags::BYPASSES_RESISTANCE,
            DamageTags::IS_FIRE,
            DamageTags::BYPASSES_COOLDOWN,
            DamageTags::BYPASSES_INVULNERABILITY,
        ])
    }

    fn hit(amount: f32, damage_type: u32) -> Hit {
        Hit::new(amount, DamageSource::of(damage_type))
    }

    fn strike(
        vitals: &mut Vitals,
        record: &mut HitRecord,
        defence: &Defence,
        hit: Hit,
    ) -> HitReport {
        hurt(
            vitals,
            record,
            defence,
            &types(),
            &HurtRules::default(),
            hit,
            10,
        )
    }

    fn plain(vitals: &mut Vitals, amount: f32) -> HitOutcome {
        let defence = Defence::default();
        strike(
            vitals,
            &mut HitRecord::default(),
            &defence,
            hit(amount, GENERIC),
        )
        .outcome
    }

    #[test]
    fn hit_003_the_cooldown_rejects_an_equal_hit_and_applies_the_rest_of_a_larger_one() {
        let mut vitals = Vitals::new(10.0);
        assert_eq!(plain(&mut vitals, 0.0), HitOutcome::Full);
        assert_eq!(
            (vitals.health, vitals.hurt_time, vitals.damage_cooldown),
            (10.0, 10, 20)
        );
        assert_eq!(plain(&mut vitals, 0.0), HitOutcome::Rejected);
        assert_eq!(plain(&mut vitals, 3.0), HitOutcome::Partial);
        assert_eq!(vitals.health, 7.0);
        vitals.damage_cooldown = 10;
        assert_eq!(
            plain(&mut vitals, 1.0),
            HitOutcome::Full,
            "the cooldown ended"
        );
        assert_eq!(vitals.health, 6.0);
    }

    #[test]
    fn hit_002_a_hit_that_the_cooldown_rejects_still_strikes_the_mob() {
        let mut vitals = Vitals::new(10.0);
        let mut record = HitRecord::default();
        let defence = Defence::default();
        let drown = hit(0.0, GENERIC);
        let outcome = |report: HitReport| report.outcome;
        assert_eq!(
            outcome(strike(&mut vitals, &mut record, &defence, drown)),
            HitOutcome::Full
        );
        let rejected = hurt(
            &mut vitals,
            &mut record,
            &defence,
            &types(),
            &HurtRules::default(),
            drown,
            12,
        );
        assert_eq!(rejected.outcome, HitOutcome::Rejected, "the cooldown runs");
        assert_eq!(record.strikes(), 2, "the rejected hit strikes the pig");
        assert_eq!(
            record.last().map(|hit| hit.tick),
            Some(10),
            "the damage record keeps the accepted hit"
        );

        let immune = Defence {
            immune: DamageTags::IS_FIRE,
            ..Defence::default()
        };
        assert_eq!(
            outcome(strike(&mut vitals, &mut record, &immune, hit(5.0, IN_FIRE))),
            HitOutcome::Rejected
        );
        assert_eq!(
            record.strikes(),
            2,
            "a hit on a fire-immune pig strikes nothing"
        );

        let invulnerable = Defence {
            invulnerable: true,
            ..Defence::default()
        };
        assert_eq!(
            outcome(strike(
                &mut vitals,
                &mut record,
                &invulnerable,
                hit(5.0, GENERIC)
            )),
            HitOutcome::Rejected
        );
        assert_eq!(
            record.strikes(),
            2,
            "a hit on an invulnerable player strikes nothing"
        );
        assert_eq!(
            outcome(strike(
                &mut vitals,
                &mut record,
                &invulnerable,
                hit(5.0, KILL)
            )),
            HitOutcome::Partial,
            "a type that bypasses invulnerability hits"
        );
        assert_eq!(vitals.health, 5.0);

        vitals.death_time = Some(1);
        assert_eq!(
            outcome(strike(&mut vitals, &mut record, &defence, drown)),
            HitOutcome::Rejected
        );
        assert_eq!(record.strikes(), 3, "a hit on a dying pig strikes nothing");
    }

    #[test]
    fn hit_001_a_creative_player_hits_an_invulnerable_entity() {
        let invulnerable = Defence {
            invulnerable: true,
            ..Defence::default()
        };
        let mut source = DamageSource::of(GENERIC);
        source.by_creative_player = true;
        let mut vitals = Vitals::new(10.0);
        let report = strike(
            &mut vitals,
            &mut HitRecord::default(),
            &invulnerable,
            Hit::new(4.0, source),
        );
        assert_eq!(report.outcome, HitOutcome::Full);
        assert_eq!(vitals.health, 6.0);

        let creative_target = Defence {
            ability_invulnerable: true,
            ..Defence::default()
        };
        let report = strike(
            &mut Vitals::new(10.0),
            &mut HitRecord::default(),
            &creative_target,
            Hit::new(4.0, source),
        );
        assert_eq!(
            report.outcome,
            HitOutcome::Rejected,
            "a creative attacker does not pass the ability invulnerability"
        );
    }

    #[test]
    fn dth_004_a_dying_mob_rejects_each_hit() {
        let mut vitals = Vitals::new(10.0);
        vitals.death_time = Some(3);
        assert_eq!(plain(&mut vitals, f32::MAX), HitOutcome::Rejected);
    }

    struct Case {
        name: &'static str,
        defence: Defence,
        absorption: f32,
        hit: Hit,
        armor: f32,
        absorbed: f32,
        health: f32,
    }

    fn reduction_cases() -> [Case; 8] {
        let armor = |armor| Defence {
            armor,
            ..Defence::default()
        };
        let resistance = |amplifier| Defence {
            resistance: Some(amplifier),
            ..Defence::default()
        };
        [
            Case {
                name: "armor 2 on magma",
                defence: armor(2.0),
                absorption: 0.0,
                hit: hit(1.0, GENERIC),
                armor: 1.0 - 0.94,
                absorbed: 0.0,
                health: 0.94,
            },
            Case {
                name: "a fraction of armor counts as nothing",
                defence: armor(2.9),
                absorption: 0.0,
                hit: hit(1.0, GENERIC),
                armor: 1.0 - 0.94,
                absorbed: 0.0,
                health: 0.94,
            },
            Case {
                name: "a fall bypasses armor",
                defence: armor(2.0),
                absorption: 0.0,
                hit: hit(1.0, FALL),
                armor: 0.0,
                absorbed: 0.0,
                health: 1.0,
            },
            Case {
                name: "resistance 0",
                defence: resistance(0),
                absorption: 0.0,
                hit: hit(5.0, GENERIC),
                armor: 1.0,
                absorbed: 0.0,
                health: 4.0,
            },
            Case {
                name: "resistance 4",
                defence: resistance(4),
                absorption: 0.0,
                hit: hit(5.0, GENERIC),
                armor: 5.0,
                absorbed: 0.0,
                health: 0.0,
            },
            Case {
                name: "a type that bypasses resistance",
                defence: resistance(4),
                absorption: 0.0,
                hit: hit(5.0, MAGIC),
                armor: 0.0,
                absorbed: 0.0,
                health: 5.0,
            },
            Case {
                name: "absorption takes the first part",
                defence: Defence::default(),
                absorption: 3.0,
                hit: hit(5.0, GENERIC),
                armor: 0.0,
                absorbed: 3.0,
                health: 2.0,
            },
            Case {
                name: "absorption takes the hit after armor",
                defence: armor(2.0),
                absorption: 3.0,
                hit: hit(1.0, GENERIC),
                armor: 1.0 - 0.94,
                absorbed: 0.94,
                health: 0.0,
            },
        ]
    }

    #[test]
    fn red_the_reductions_and_the_absorption_split_the_hit() {
        for case in reduction_cases() {
            let mut vitals = Vitals::new(20.0);
            vitals.absorption = case.absorption;
            let report = strike(
                &mut vitals,
                &mut HitRecord::default(),
                &case.defence,
                case.hit,
            );
            assert_eq!(
                (report.armor, (report.absorption, report.health)),
                (case.armor, (case.absorbed, case.health)),
                "{}",
                case.name
            );
            assert_eq!(vitals.health, 20.0 - case.health, "{}", case.name);
            assert_eq!(
                vitals.absorption,
                case.absorption - case.absorbed,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn ord_003_two_hits_in_one_tick_change_the_health_alike_in_each_order() {
        let cooling = Vitals {
            damage_cooldown: 15,
            last_damage: 2.0,
            ..Vitals::new(20.0)
        };
        let cases = [
            (
                Vitals::new(20.0),
                [hit(3.0, GENERIC), hit(5.0, GENERIC)],
                15.0,
                15.0,
            ),
            (cooling, [hit(3.0, GENERIC), hit(5.0, GENERIC)], 17.0, 17.0),
            (
                Vitals::new(20.0),
                [hit(2.0, ALWAYS), hit(5.0, GENERIC)],
                15.0,
                13.0,
            ),
        ];
        let defence = Defence::default();
        for (start, [first, second], forward, backward) in cases {
            let health = |hits: [Hit; 2]| {
                let mut vitals = start;
                let mut record = HitRecord::default();
                for hit in hits {
                    let _ = strike(&mut vitals, &mut record, &defence, hit);
                }
                vitals.health
            };
            assert_eq!(
                (health([first, second]), health([second, first])),
                (forward, backward),
                "{first:?} and {second:?} on {start:?}"
            );
        }
    }
}
