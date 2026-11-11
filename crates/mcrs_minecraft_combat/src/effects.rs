use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::prelude::*;
use mcrs_minecraft_damage::{DamageSource, Hit};

use crate::events::DamageRequest;
use crate::health::Health;
use crate::hunger::FoodData;

/// The standard mob effects in Minecraft (matching `minecraft:mob_effect` registry).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum MobEffect {
    Speed = 0,
    Slowness = 1,
    Haste = 2,
    MiningFatigue = 3,
    Strength = 4,
    InstantHealth = 5,
    InstantDamage = 6,
    JumpBoost = 7,
    Nausea = 8,
    Regeneration = 9,
    Resistance = 10,
    FireResistance = 11,
    WaterBreathing = 12,
    Invisibility = 13,
    Blindness = 14,
    NightVision = 15,
    Hunger = 16,
    Weakness = 17,
    Poison = 18,
    Wither = 19,
    HealthBoost = 20,
    Absorption = 21,
    Saturation = 22,
    Glowing = 23,
    Levitation = 24,
    Luck = 25,
    Unluck = 26,
    SlowFalling = 27,
    ConduitPower = 28,
    DolphinsGrace = 29,
    BadOmen = 30,
    HeroOfTheVillage = 31,
    Darkness = 32,
    TrialOmen = 33,
    RaidOmen = 34,
    WindCharged = 35,
    Weaving = 36,
    Oozing = 37,
    Infested = 38,
    BreathOfTheNautilus = 39,
    Freezing = 40,
}

impl MobEffect {
    #[must_use]
    pub const fn id(self) -> u32 {
        self as u32
    }

    #[must_use]
    pub const fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::Speed),
            1 => Some(Self::Slowness),
            2 => Some(Self::Haste),
            3 => Some(Self::MiningFatigue),
            4 => Some(Self::Strength),
            5 => Some(Self::InstantHealth),
            6 => Some(Self::InstantDamage),
            7 => Some(Self::JumpBoost),
            8 => Some(Self::Nausea),
            9 => Some(Self::Regeneration),
            10 => Some(Self::Resistance),
            11 => Some(Self::FireResistance),
            12 => Some(Self::WaterBreathing),
            13 => Some(Self::Invisibility),
            14 => Some(Self::Blindness),
            15 => Some(Self::NightVision),
            16 => Some(Self::Hunger),
            17 => Some(Self::Weakness),
            18 => Some(Self::Poison),
            19 => Some(Self::Wither),
            20 => Some(Self::HealthBoost),
            21 => Some(Self::Absorption),
            22 => Some(Self::Saturation),
            23 => Some(Self::Glowing),
            24 => Some(Self::Levitation),
            25 => Some(Self::Luck),
            26 => Some(Self::Unluck),
            27 => Some(Self::SlowFalling),
            28 => Some(Self::ConduitPower),
            29 => Some(Self::DolphinsGrace),
            30 => Some(Self::BadOmen),
            31 => Some(Self::HeroOfTheVillage),
            32 => Some(Self::Darkness),
            33 => Some(Self::TrialOmen),
            34 => Some(Self::RaidOmen),
            35 => Some(Self::WindCharged),
            36 => Some(Self::Weaving),
            37 => Some(Self::Oozing),
            38 => Some(Self::Infested),
            39 => Some(Self::BreathOfTheNautilus),
            40 => Some(Self::Freezing),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_instant(self) -> bool {
        matches!(self, Self::InstantHealth | Self::InstantDamage)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Speed => "minecraft:speed",
            Self::Slowness => "minecraft:slowness",
            Self::Haste => "minecraft:haste",
            Self::MiningFatigue => "minecraft:mining_fatigue",
            Self::Strength => "minecraft:strength",
            Self::InstantHealth => "minecraft:instant_health",
            Self::InstantDamage => "minecraft:instant_damage",
            Self::JumpBoost => "minecraft:jump_boost",
            Self::Nausea => "minecraft:nausea",
            Self::Regeneration => "minecraft:regeneration",
            Self::Resistance => "minecraft:resistance",
            Self::FireResistance => "minecraft:fire_resistance",
            Self::WaterBreathing => "minecraft:water_breathing",
            Self::Invisibility => "minecraft:invisibility",
            Self::Blindness => "minecraft:blindness",
            Self::NightVision => "minecraft:night_vision",
            Self::Hunger => "minecraft:hunger",
            Self::Weakness => "minecraft:weakness",
            Self::Poison => "minecraft:poison",
            Self::Wither => "minecraft:wither",
            Self::HealthBoost => "minecraft:health_boost",
            Self::Absorption => "minecraft:absorption",
            Self::Saturation => "minecraft:saturation",
            Self::Glowing => "minecraft:glowing",
            Self::Levitation => "minecraft:levitation",
            Self::Luck => "minecraft:luck",
            Self::Unluck => "minecraft:unluck",
            Self::SlowFalling => "minecraft:slow_falling",
            Self::ConduitPower => "minecraft:conduit_power",
            Self::DolphinsGrace => "minecraft:dolphins_grace",
            Self::BadOmen => "minecraft:bad_omen",
            Self::HeroOfTheVillage => "minecraft:hero_of_the_village",
            Self::Darkness => "minecraft:darkness",
            Self::TrialOmen => "minecraft:trial_omen",
            Self::RaidOmen => "minecraft:raid_omen",
            Self::WindCharged => "minecraft:wind_charged",
            Self::Weaving => "minecraft:weaving",
            Self::Oozing => "minecraft:oozing",
            Self::Infested => "minecraft:infested",
            Self::BreathOfTheNautilus => "minecraft:breath_of_the_nautilus",
            Self::Freezing => "minecraft:freezing",
        }
    }
}

/// A single active effect instance on an entity.
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

/// Component storing active status effects on an entity.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ActiveEffects {
    effects: Vec<ActiveEffect>,
}

impl ActiveEffects {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Adds or updates an active effect.
    ///
    /// If an effect of the same type already exists, it is replaced if the new
    /// amplifier is higher, or if the amplifier is identical and the new duration is longer.
    pub fn add(&mut self, new_effect: ActiveEffect) -> bool {
        if let Some(existing) = self.effects.iter_mut().find(|e| e.effect == new_effect.effect) {
            if new_effect.amplifier > existing.amplifier
                || (new_effect.amplifier == existing.amplifier && new_effect.duration > existing.duration)
            {
                *existing = new_effect;
                true
            } else {
                false
            }
        } else {
            self.effects.push(new_effect);
            true
        }
    }

    /// Removes an active effect by type.
    pub fn remove(&mut self, effect: MobEffect) -> Option<ActiveEffect> {
        if let Some(pos) = self.effects.iter().position(|e| e.effect == effect) {
            Some(self.effects.remove(pos))
        } else {
            None
        }
    }

    #[must_use]
    pub fn get(&self, effect: MobEffect) -> Option<&ActiveEffect> {
        self.effects.iter().find(|e| e.effect == effect)
    }

    #[must_use]
    pub fn has(&self, effect: MobEffect) -> bool {
        self.effects.iter().any(|e| e.effect == effect)
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

fn apply_single_effect_tick(
    entity: Entity,
    effect: &ActiveEffect,
    mut health: Option<&mut Health>,
    mut food: Option<&mut FoodData>,
    damage_events: &mut MessageWriter<DamageRequest>,
) {
    let amplifier = effect.amplifier;
    let duration = effect.duration;

    match effect.effect {
        MobEffect::Regeneration => {
            let interval = (50 >> amplifier).max(1);
            if duration.is_multiple_of(interval)
                && let Some(ref mut h) = health
            {
                h.heal(1.0);
            }
        }
        MobEffect::Poison => {
            let interval = (25 >> amplifier).max(1);
            if duration.is_multiple_of(interval)
                && let Some(ref h) = health
                && h.current > 1.0
            {
                damage_events.write(DamageRequest::new(
                    entity,
                    Hit::from_source(1.0, DamageSource::new(0)),
                ));
            }
        }
        MobEffect::Wither => {
            let interval = (40 >> amplifier).max(1);
            if duration.is_multiple_of(interval) {
                damage_events.write(DamageRequest::new(
                    entity,
                    Hit::from_source(1.0, DamageSource::new(0)),
                ));
            }
        }
        MobEffect::Hunger => {
            if let Some(ref mut f) = food {
                f.add_exhaustion(0.005 * f32::from(amplifier.saturating_add(1)));
            }
        }
        MobEffect::Saturation => {
            if let Some(ref mut f) = food {
                let add_food = u32::from(amplifier.saturating_add(1));
                let add_sat = f32::from(amplifier.saturating_add(1)) * 2.0;
                f.food_level = f.food_level.saturating_add(add_food).min(20);
                f.saturation = (f.saturation + add_sat).min(f.food_level as f32);
            }
        }
        _ => {}
    }
}

/// Advances active status effects by one tick, applying periodic ticks (regeneration,
/// poison, wither, hunger, saturation) and expiring finished effects.
pub fn tick_active_effects(
    mut query: Query<(
        Entity,
        &mut ActiveEffects,
        Option<&mut Health>,
        Option<&mut FoodData>,
    )>,
    mut damage_events: MessageWriter<DamageRequest>,
    mut remove_events: MessageWriter<RemoveEffectEvent>,
) {
    for (entity, mut active, mut health_opt, mut food_opt) in &mut query {
        let mut to_remove = Vec::new();
        for effect in &mut active.effects {
            apply_single_effect_tick(
                entity,
                effect,
                health_opt.as_deref_mut(),
                food_opt.as_deref_mut(),
                &mut damage_events,
            );
            effect.duration = effect.duration.saturating_sub(1);
            if effect.duration == 0 {
                to_remove.push(effect.effect);
            }
        }

        for removed in to_remove {
            active.remove(removed);
            remove_events.write(RemoveEffectEvent {
                entity,
                effect: removed,
            });
        }
    }
}

/// Applies instant health and instant damage immediately.
pub fn handle_apply_effect_events(
    mut events: MessageReader<ApplyEffectEvent>,
    mut query: Query<(&mut Health, Option<&mut ActiveEffects>)>,
    mut damage_events: MessageWriter<DamageRequest>,
) {
    for ev in events.read() {
        if let Ok((mut health, effects_opt)) = query.get_mut(ev.entity) {
            let amp_shift = ev.effect.amplifier.min(15);
            match ev.effect.effect {
                MobEffect::InstantHealth => {
                    let amount = 4.0 * f32::from(1u16 << amp_shift);
                    health.heal(amount);
                }
                MobEffect::InstantDamage => {
                    let amount = 6.0 * f32::from(1u16 << amp_shift);
                    damage_events.write(DamageRequest::new(
                        ev.entity,
                        Hit::from_source(amount, DamageSource::new(0)),
                    ));
                }
                _ => {
                    if let Some(mut effects) = effects_opt {
                        effects.add(ev.effect);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};

    #[test]
    fn test_active_effects_add_and_replace() {
        let mut effects = ActiveEffects::new();
        let eff1 = ActiveEffect::new(MobEffect::Speed, 0, 100);
        assert!(effects.add(eff1));
        assert_eq!(effects.len(), 1);

        // Lower amplifier does not replace
        let eff2 = ActiveEffect::new(MobEffect::Speed, 0, 50);
        assert!(!effects.add(eff2));
        assert_eq!(effects.get(MobEffect::Speed).map(|e| e.duration), Some(100));

        // Higher amplifier replaces
        let eff3 = ActiveEffect::new(MobEffect::Speed, 1, 50);
        assert!(effects.add(eff3));
        assert_eq!(effects.get(MobEffect::Speed).map(|e| e.amplifier), Some(1));
        assert_eq!(effects.get(MobEffect::Speed).map(|e| e.duration), Some(50));
    }

    #[test]
    fn test_tick_active_effects_regeneration() {
        let mut app = App::new();
        app.add_message::<DamageRequest>();
        app.add_message::<RemoveEffectEvent>();
        app.add_systems(Update, tick_active_effects);

        let mut effects = ActiveEffects::new();
        effects.add(ActiveEffect::new(MobEffect::Regeneration, 0, 50));

        let mut health = Health::new(20.0);
        health.current = 10.0;

        let entity = app
            .world_mut()
            .spawn((effects, health))
            .id();

        app.update();

        let health = app.world().get::<Health>(entity).unwrap();
        // Since duration 50 % 50 == 0, regeneration heals 1.0!
        assert_eq!(health.current, 11.0);
    }
}
