use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_physics::MotionEffects;

/// One active effect on a living entity (EFX-INS-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveEffect {
    pub effect: MobEffect,
    pub amplifier: u8,
    pub duration: u32,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
}

impl ActiveEffect {
    #[must_use]
    pub const fn new(effect: MobEffect, amplifier: u8, duration: u32) -> Self {
        Self {
            effect,
            amplifier,
            duration,
            ambient: false,
            show_particles: true,
            show_icon: true,
        }
    }

    #[must_use]
    pub const fn with_ambient(mut self, ambient: bool) -> Self {
        self.ambient = ambient;
        self
    }

    #[must_use]
    pub const fn with_particles(mut self, show_particles: bool) -> Self {
        self.show_particles = show_particles;
        self
    }

    #[must_use]
    pub const fn with_icon(mut self, show_icon: bool) -> Self {
        self.show_icon = show_icon;
        self
    }
}

/// What one effect tick asks its owner to do (EFX-ACT-001 to EFX-ACT-010).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectAction {
    Heal(f32),
    Damage { magic: bool, amount: f32 },
    Exhaustion(f32),
    Saturation { food: u32, saturation: f32 },
}

/// The active effects of a living entity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    effects: Vec<ActiveEffect>,
}

impl Effects {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Adds or merges `new_effect` (EFX-ADD-001, EFX-ADD-002).
    pub fn add(&mut self, new_effect: ActiveEffect) -> bool {
        if new_effect.duration == 0 {
            return false;
        }
        if let Some(existing) = self
            .effects
            .iter_mut()
            .find(|existing| existing.effect == new_effect.effect)
        {
            let stronger = new_effect.amplifier > existing.amplifier
                || (new_effect.amplifier == existing.amplifier
                    && new_effect.duration > existing.duration);
            if stronger {
                *existing = new_effect;
            }
            stronger
        } else {
            self.effects.push(new_effect);
            true
        }
    }

    pub fn insert(&mut self, effect: ActiveEffect) {
        if let Some(existing) = self
            .effects
            .iter_mut()
            .find(|existing| existing.effect == effect.effect)
        {
            *existing = effect;
        } else {
            self.effects.push(effect);
        }
    }

    pub fn remove(&mut self, effect: MobEffect) -> Option<ActiveEffect> {
        let at = self
            .effects
            .iter()
            .position(|active| active.effect == effect)?;
        Some(self.effects.remove(at))
    }

    #[must_use]
    pub fn get(&self, effect: MobEffect) -> Option<&ActiveEffect> {
        self.effects.iter().find(|active| active.effect == effect)
    }

    #[must_use]
    pub fn has(&self, effect: MobEffect) -> bool {
        self.get(effect).is_some()
    }

    #[must_use]
    pub fn amplifier(&self, effect: MobEffect) -> Option<u8> {
        self.get(effect).map(|active| active.amplifier)
    }

    #[must_use]
    pub fn movement_effects(&self) -> MotionEffects {
        MotionEffects {
            speed: self.amplifier(MobEffect::Speed),
            slowness: self.amplifier(MobEffect::Slowness),
            slow_falling: self.has(MobEffect::SlowFalling),
            levitation: self.amplifier(MobEffect::Levitation),
            jump_boost: self.amplifier(MobEffect::JumpBoost),
            weaving: self.has(MobEffect::Weaving),
            blindness: self.has(MobEffect::Blindness),
        }
    }

    #[must_use]
    pub fn attack_damage_bonus(&self) -> f32 {
        let strength = self
            .amplifier(MobEffect::Strength)
            .map_or(0.0, |level| 3.0 * f32::from(level.saturating_add(1)));
        let weakness = self
            .amplifier(MobEffect::Weakness)
            .map_or(0.0, |level| 4.0 * f32::from(level.saturating_add(1)));
        strength - weakness
    }

    #[must_use]
    pub fn resistance(&self) -> Option<u8> {
        self.amplifier(MobEffect::Resistance)
    }

    #[must_use]
    pub fn absorption_cap(&self) -> f32 {
        self.amplifier(MobEffect::Absorption)
            .map_or(0.0, |level| 4.0 * f32::from(level.saturating_add(1)))
    }

    #[must_use]
    pub fn health_boost(&self) -> f32 {
        self.amplifier(MobEffect::HealthBoost)
            .map_or(0.0, |level| 4.0 * f32::from(level.saturating_add(1)))
    }

    /// Advances one effect tick (EFX-TCK-001, EFX-TCK-002). `undead` swaps the instant effects,
    /// and `absorption` decides whether the absorption effect ends.
    pub fn tick(
        &mut self,
        undead: bool,
        health: f32,
        absorption: f32,
        mut action: impl FnMut(EffectAction),
    ) -> Vec<MobEffect> {
        let mut remove = Vec::new();
        for active in &mut self.effects {
            let duration = active.duration;
            let amplifier = active.amplifier;
            match active.effect {
                MobEffect::Regeneration => {
                    let interval = 50u32 >> amplifier.min(31);
                    if interval == 0 || duration.is_multiple_of(interval) {
                        action(EffectAction::Heal(1.0));
                    }
                }
                MobEffect::Poison => {
                    let interval = 25u32 >> amplifier.min(31);
                    if (interval == 0 || duration.is_multiple_of(interval)) && health > 1.0 {
                        action(EffectAction::Damage {
                            magic: true,
                            amount: 1.0,
                        });
                    }
                }
                MobEffect::Wither => {
                    let interval = 40u32 >> amplifier.min(31);
                    if interval == 0 || duration.is_multiple_of(interval) {
                        action(EffectAction::Damage {
                            magic: false,
                            amount: 1.0,
                        });
                    }
                }
                MobEffect::InstantHealth => {
                    let amount = 4.0 * f32::from(1u16 << amplifier.min(15));
                    if undead {
                        action(EffectAction::Damage {
                            magic: true,
                            amount: amount * 1.5,
                        });
                    } else {
                        action(EffectAction::Heal(amount));
                    }
                }
                MobEffect::InstantDamage => {
                    let amount = 6.0 * f32::from(1u16 << amplifier.min(15));
                    if undead {
                        action(EffectAction::Heal(amount * 2.0 / 3.0));
                    } else {
                        action(EffectAction::Damage {
                            magic: true,
                            amount,
                        });
                    }
                }
                MobEffect::Hunger => {
                    action(EffectAction::Exhaustion(
                        0.005 * f32::from(amplifier.saturating_add(1)),
                    ));
                }
                MobEffect::Saturation => {
                    action(EffectAction::Saturation {
                        food: u32::from(amplifier.saturating_add(1)),
                        saturation: f32::from(amplifier.saturating_add(1)) * 2.0,
                    });
                }
                MobEffect::Absorption if absorption <= 0.0 => {
                    remove.push(MobEffect::Absorption);
                }
                _ => {}
            }
            active.duration = active.duration.saturating_sub(1);
            if active.duration == 0 && !remove.contains(&active.effect) {
                remove.push(active.effect);
            }
        }
        for effect in &remove {
            self.remove(*effect);
        }
        remove
    }

    pub fn iter(&self) -> impl Iterator<Item = &ActiveEffect> {
        self.effects.iter()
    }

    pub fn clear(&mut self) {
        self.effects.clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stronger_effect_replaces_a_weaker_one() {
        let mut effects = Effects::new();
        assert!(effects.add(ActiveEffect::new(MobEffect::Speed, 0, 100)));
        assert!(!effects.add(ActiveEffect::new(MobEffect::Speed, 0, 50)));
        assert!(effects.add(ActiveEffect::new(MobEffect::Speed, 1, 50)));
        assert_eq!(effects.amplifier(MobEffect::Speed), Some(1));
    }

    #[test]
    fn regeneration_acts_every_fifty_ticks() {
        let mut effects = Effects::new();
        effects.add(ActiveEffect::new(MobEffect::Regeneration, 0, 50));
        let mut healed = 0.0;
        for _ in 0..50 {
            let _ = effects.tick(false, 20.0, 0.0, |action| {
                if let EffectAction::Heal(amount) = action {
                    healed += amount;
                }
            });
        }
        assert_eq!(healed, 1.0);
        assert!(effects.is_empty());
    }

    #[test]
    fn resistance_is_a_fifth_per_level() {
        let mut effects = Effects::new();
        effects.add(ActiveEffect::new(MobEffect::Resistance, 0, 100));
        assert_eq!(effects.resistance(), Some(0));
    }
}
