use bevy_ecs::component::Component;
use core::ops::{Deref, DerefMut};
use mcrs_minecraft_damage::{Effects, Experience, Food};
use mcrs_minecraft_engine::player_state::{Attribute, EffectData};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_physics::HitRecord;
use mcrs_minecraft_physics::Vitals;
use mcrs_minecraft_resource::ResourceKeyBuf;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The health, the air, the fire and the freeze state of a player (DMG-ATT-001).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PlayerVitals(pub Vitals);

/// The max health of a player before the modifiers of its effects.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BaseMaxHealth(pub f32);

impl Default for BaseMaxHealth {
    fn default() -> Self {
        Self(20.0)
    }
}

impl Default for PlayerVitals {
    fn default() -> Self {
        Self(Vitals {
            health: 20.0,
            ..Vitals::default()
        })
    }
}

impl PlayerVitals {
    #[must_use]
    pub const fn new(health: f32, max_health: f32) -> Self {
        Self(Vitals {
            health,
            max_health,
            absorption: 0.0,
            last_damage: 0.0,
            damage_cooldown: 0,
            hurt_time: 0,
            death_time: None,
            fire_ticks: 0,
            air: 300,
            frozen_ticks: 0,
        })
    }

    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.0.death_time.is_none()
    }

    pub fn heal(&mut self, amount: f32) {
        if self.is_alive() {
            self.0.health = (self.0.health + amount).min(self.0.max_health);
        }
    }
}

impl Deref for PlayerVitals {
    type Target = Vitals;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PlayerVitals {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The hunger state of a player (HNG-STA-001).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerFood(pub Food);

impl Deref for PlayerFood {
    type Target = Food;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PlayerFood {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The active effects of a player (EFX-INS-001).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct PlayerEffects(pub Effects);

impl PlayerEffects {
    #[must_use]
    pub const fn new() -> Self {
        Self(Effects::new())
    }
}

impl Deref for PlayerEffects {
    type Target = Effects;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PlayerEffects {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The experience of a player (XP-STA-001).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerExperience(pub Experience);

impl Deref for PlayerExperience {
    type Target = Experience;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PlayerExperience {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The record of the last hit on a player (DMG-HIT-002).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerHitRecord(pub HitRecord);

/// The point where a player respawns after a death.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct RespawnPoint {
    pub dimension: ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    pub position: Transform,
    pub forced: bool,
}

/// The fall distance and the ground flag of a player (DMG-ENV-001).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerEnvironment {
    pub fall_distance: f64,
    pub on_ground: bool,
}

/// The attributes of a player, as the player file holds them.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerAttributes(pub Vec<Attribute>);

impl PlayerAttributes {
    #[must_use]
    pub fn max_health_base(&self) -> f32 {
        self.0
            .iter()
            .find(|attribute| {
                attribute.id == "minecraft:max_health" || attribute.id == "max_health"
            })
            .map_or(20.0, |attribute| attribute.base as f32)
    }

    pub fn set_max_health_base(&mut self, base: f32) {
        if let Some(attribute) = self.0.iter_mut().find(|attribute| {
            attribute.id == "minecraft:max_health" || attribute.id == "max_health"
        }) {
            attribute.base = f64::from(base);
        } else {
            self.0.push(Attribute {
                id: "minecraft:max_health".to_owned(),
                base: f64::from(base),
                modifiers: Vec::new(),
            });
        }
    }
}

/// The hidden effects of the active effects, which the player file keeps below each effect.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct PlayerEffectsHidden(pub BTreeMap<MobEffect, EffectData>);

/// The marker of a player in its dying state (DMG-DTH-001).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dying;

/// The armor and toughness that the hit reduction reads (DMG-RED-001).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Armor {
    pub armor: f32,
    pub toughness: f32,
}

/// The knockback resistance of a player, from 0 to 1.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct KnockbackResistance(pub f64);

/// The armor slots of a player.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ArmorToughness(pub f32);

/// The attack damage that the equipment of a player adds.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackDamage(pub f32);
