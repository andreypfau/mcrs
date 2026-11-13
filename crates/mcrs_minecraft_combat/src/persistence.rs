use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_damage::{ActiveEffect, Effects};
use mcrs_minecraft_engine::persisted_registry::{
    PersistScope, PersistedAppExt, PersistedError, PersistedForm,
};
use mcrs_minecraft_engine::player_state::{Effect, EffectData, Ints};
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::tag_deserializer::from_tag;
use mcrs_minecraft_nbt::tag_serializer::to_nbt_tag;
use mcrs_minecraft_physics::Vitals;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::components::{
    PlayerAttributes, PlayerEffects, PlayerEffectsHidden, PlayerEnvironment, PlayerExperience,
    PlayerFood, PlayerVitals, RespawnPoint,
};

pub const VITALS_KEYS: &[&str] = &[
    "Health",
    "HurtTime",
    "DeathTime",
    "AbsorptionAmount",
    "Fire",
    "Air",
    "TicksFrozen",
];
pub const ENVIRONMENT_KEYS: &[&str] = &["fall_distance", "OnGround"];
pub const FOOD_KEYS: &[&str] = &[
    "foodLevel",
    "foodTickTimer",
    "foodSaturationLevel",
    "foodExhaustionLevel",
];
pub const EXPERIENCE_KEYS: &[&str] = &["XpLevel", "XpTotal", "Score", "XpP"];
pub const ATTRIBUTES_KEYS: &[&str] = &["attributes"];
pub const EFFECTS_KEYS: &[&str] = &["active_effects"];
pub const RESPAWN_KEYS: &[&str] = &["respawn"];

#[derive(Clone, Copy, Debug)]
pub struct CombatPersistencePlugin;

impl Plugin for CombatPersistencePlugin {
    fn build(&self, app: &mut App) {
        register(app);
    }
}

pub fn register(app: &mut App) {
    app.register_persisted::<PlayerAttributes>(ATTRIBUTES_KEYS, PersistScope::Persisted)
        .register_persisted_codec(
            VITALS_KEYS,
            PersistScope::Persisted,
            capture_vitals,
            apply_vitals,
        )
        .register_persisted_as::<PlayerEnvironment, EnvironmentForm>(
            ENVIRONMENT_KEYS,
            PersistScope::Persisted,
        )
        .register_persisted_as::<PlayerFood, FoodForm>(FOOD_KEYS, PersistScope::Persisted)
        .register_persisted_as::<PlayerExperience, ExperienceForm>(
            EXPERIENCE_KEYS,
            PersistScope::Persisted,
        )
        .register_persisted_as::<RespawnPoint, RespawnForm>(RESPAWN_KEYS, PersistScope::Persisted)
        .register_persisted_codec(
            EFFECTS_KEYS,
            PersistScope::Persisted,
            capture_effects,
            apply_effects,
        );
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
struct VitalsForm {
    #[serde(rename = "Health")]
    health: f32,
    #[serde(rename = "HurtTime")]
    hurt_time: i16,
    #[serde(rename = "DeathTime")]
    death_time: i16,
    #[serde(rename = "AbsorptionAmount")]
    absorption_amount: f32,
    #[serde(rename = "Fire")]
    fire: i16,
    #[serde(rename = "Air")]
    air: i16,
    #[serde(rename = "TicksFrozen", skip_serializing_if = "is_zero")]
    ticks_frozen: i32,
}

impl Default for VitalsForm {
    fn default() -> Self {
        Self {
            health: 20.0,
            hurt_time: 0,
            death_time: 0,
            absorption_amount: 0.0,
            fire: 0,
            air: 300,
            ticks_frozen: 0,
        }
    }
}

fn capture_vitals(world: &mut World, entity: Entity) -> Result<Option<NbtTag>, PersistedError> {
    let Some(vitals) = world.get::<PlayerVitals>(entity) else {
        return Ok(None);
    };
    to_nbt_tag(&VitalsForm::from_value(vitals))
        .map(Some)
        .map_err(|source| PersistedError::Encode {
            key: "Health",
            source,
        })
}

fn apply_vitals(world: &mut World, entity: Entity, tag: &NbtTag) -> Result<(), PersistedError> {
    let form: VitalsForm = from_tag(tag.clone()).map_err(|source| PersistedError::Decode {
        key: "Health",
        source,
    })?;
    let has_health = match tag {
        NbtTag::Compound(compound) => compound.get("Health").is_some(),
        _ => false,
    };
    let mut vitals = form.into_value().map_err(|source| PersistedError::Decode {
        key: "Health",
        source: serde::de::Error::custom(source),
    })?;
    if !has_health {
        vitals.0.health = world
            .get::<PlayerAttributes>(entity)
            .map_or(20.0, PlayerAttributes::max_health_base);
    }
    world
        .get_entity_mut(entity)
        .map_err(|_| PersistedError::NoEntity(entity))?
        .insert(vitals);
    Ok(())
}

impl PersistedForm<PlayerVitals> for VitalsForm {
    fn from_value(vitals: &PlayerVitals) -> Self {
        Self {
            health: vitals.health,
            hurt_time: i16::from(vitals.hurt_time),
            death_time: vitals.death_time.map_or(0, i16::from),
            absorption_amount: vitals.absorption,
            fire: i16::try_from(vitals.fire_ticks).unwrap_or(if vitals.fire_ticks < 0 {
                i16::MIN
            } else {
                i16::MAX
            }),
            air: i16::try_from(vitals.air).unwrap_or(if vitals.air < 0 {
                i16::MIN
            } else {
                i16::MAX
            }),
            ticks_frozen: vitals.frozen_ticks,
        }
    }

    fn into_value(self) -> Result<PlayerVitals, String> {
        let health = self.health.max(0.0);
        let death_time = if self.death_time > 0 {
            Some(u8::try_from(self.death_time).unwrap_or(u8::MAX))
        } else if health <= 0.0 {
            Some(0)
        } else {
            None
        };
        Ok(PlayerVitals(Vitals {
            health,
            max_health: health.max(20.0),
            absorption: self.absorption_amount.max(0.0),
            last_damage: 0.0,
            damage_cooldown: 0,
            hurt_time: u8::try_from(self.hurt_time).unwrap_or(u8::MAX),
            death_time,
            fire_ticks: i32::from(self.fire),
            air: i32::from(self.air).max(0),
            frozen_ticks: self.ticks_frozen.max(0),
        }))
    }
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct EnvironmentForm {
    fall_distance: f64,
    #[serde(rename = "OnGround")]
    on_ground: bool,
}

impl PersistedForm<PlayerEnvironment> for EnvironmentForm {
    fn from_value(environment: &PlayerEnvironment) -> Self {
        Self {
            fall_distance: environment.fall_distance,
            on_ground: environment.on_ground,
        }
    }

    fn into_value(self) -> Result<PlayerEnvironment, String> {
        Ok(PlayerEnvironment {
            fall_distance: self.fall_distance.max(0.0),
            on_ground: self.on_ground,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
struct FoodForm {
    #[serde(rename = "foodLevel")]
    food_level: i32,
    #[serde(rename = "foodTickTimer")]
    food_tick_timer: i32,
    #[serde(rename = "foodSaturationLevel")]
    food_saturation_level: f32,
    #[serde(rename = "foodExhaustionLevel")]
    food_exhaustion_level: f32,
}

impl Default for FoodForm {
    fn default() -> Self {
        Self {
            food_level: 20,
            food_tick_timer: 0,
            food_saturation_level: 5.0,
            food_exhaustion_level: 0.0,
        }
    }
}

impl PersistedForm<PlayerFood> for FoodForm {
    fn from_value(food: &PlayerFood) -> Self {
        Self {
            food_level: i32::try_from(food.food_level).unwrap_or(i32::MAX),
            food_tick_timer: i32::try_from(food.tick_timer).unwrap_or(i32::MAX),
            food_saturation_level: food.saturation,
            food_exhaustion_level: food.exhaustion,
        }
    }

    fn into_value(self) -> Result<PlayerFood, String> {
        let food_level = u32::try_from(self.food_level).unwrap_or(0).min(20);
        Ok(PlayerFood(mcrs_minecraft_damage::Food {
            food_level,
            saturation: self.food_saturation_level.max(0.0).min(food_level as f32),
            exhaustion: self.food_exhaustion_level.max(0.0),
            tick_timer: u32::try_from(self.food_tick_timer).unwrap_or(0),
        }))
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ExperienceForm {
    #[serde(rename = "XpLevel")]
    level: i32,
    #[serde(rename = "XpTotal")]
    total: i32,
    #[serde(rename = "Score")]
    score: i32,
    #[serde(rename = "XpP")]
    progress: f32,
}

impl PersistedForm<PlayerExperience> for ExperienceForm {
    fn from_value(experience: &PlayerExperience) -> Self {
        Self {
            level: experience.level,
            total: experience.total,
            score: experience.score,
            progress: experience.progress,
        }
    }

    fn into_value(self) -> Result<PlayerExperience, String> {
        Ok(PlayerExperience(mcrs_minecraft_damage::Experience {
            level: self.level.max(0),
            progress: self.progress.max(0.0),
            total: self.total.max(0),
            score: self.score,
        }))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RespawnForm {
    dimension: String,
    pos: Ints<3>,
    yaw: f32,
    pitch: f32,
    #[serde(default)]
    forced: bool,
}

impl PersistedForm<RespawnPoint> for RespawnForm {
    fn from_value(point: &RespawnPoint) -> Self {
        let [x, y, z] = point.position.translation.to_array();
        Self {
            dimension: point.dimension.as_str().to_owned(),
            pos: Ints([x.floor() as i32, y.floor() as i32, z.floor() as i32]),
            yaw: point.position.rotation.yaw(),
            pitch: point.position.rotation.pitch(),
            forced: point.forced,
        }
    }

    fn into_value(self) -> Result<RespawnPoint, String> {
        let location = ResourceLocationBuf::read(&self.dimension)
            .map_err(|error| format!("respawn dimension: {error}"))?;
        let [x, y, z] = self.pos.0;
        Ok(RespawnPoint {
            dimension: ResourceKeyBuf::from_location(location),
            position: mcrs_minecraft_engine_world::entity::physics::Transform::from_translation(
                bevy_math::DVec3::new(f64::from(x), f64::from(y), f64::from(z)),
            )
            .with_rotation(
                mcrs_minecraft_engine_world::entity::physics::Rotation::new(self.yaw, self.pitch),
            ),
            forced: self.forced,
        })
    }
}

fn capture_effects(world: &mut World, entity: Entity) -> Result<Option<NbtTag>, PersistedError> {
    let Some(effects) = world.get::<PlayerEffects>(entity) else {
        return Ok(None);
    };
    if effects.is_empty() {
        return Ok(None);
    }
    let hidden = world.get::<PlayerEffectsHidden>(entity);
    let saved: Vec<Effect> = effects
        .iter()
        .map(|active| saved_effect(active, hidden))
        .collect();
    to_nbt_tag(&saved)
        .map(Some)
        .map_err(|source| PersistedError::Encode {
            key: "active_effects",
            source,
        })
}

fn apply_effects(world: &mut World, entity: Entity, tag: &NbtTag) -> Result<(), PersistedError> {
    let saved: Vec<Effect> = from_tag(tag.clone()).map_err(|source| PersistedError::Decode {
        key: "active_effects",
        source,
    })?;
    let (effects, hidden) = load_effects(&saved);
    world
        .get_entity_mut(entity)
        .map_err(|_| PersistedError::NoEntity(entity))?
        .insert((PlayerEffects(effects), PlayerEffectsHidden(hidden)));
    Ok(())
}

fn saved_effect(active: &ActiveEffect, hidden: Option<&PlayerEffectsHidden>) -> Effect {
    Effect {
        id: active.effect.as_static_str().to_owned(),
        amplifier: active.amplifier,
        duration: if active.duration == u32::MAX {
            -1
        } else {
            i32::try_from(active.duration).unwrap_or(i32::MAX)
        },
        ambient: active.ambient,
        show_particles: active.show_particles,
        show_icon: Some(active.show_icon),
        hidden_effect: hidden
            .and_then(|hidden| hidden.0.get(&active.effect))
            .map(|effect| Box::new(effect.clone())),
    }
}

fn load_effects(saved: &[Effect]) -> (Effects, BTreeMap<MobEffect, EffectData>) {
    let mut effects = Effects::new();
    let mut hidden = BTreeMap::new();
    for saved in saved {
        let Some(effect) = MobEffect::read(&saved.id) else {
            continue;
        };
        if saved.duration == 0 {
            continue;
        }
        effects.insert(active_from_saved(effect, saved));
        hidden.remove(&effect);
        if let Some(data) = &saved.hidden_effect {
            hidden.insert(effect, data.as_ref().clone());
        }
    }
    (effects, hidden)
}

fn active_from_saved(effect: MobEffect, saved: &Effect) -> ActiveEffect {
    ActiveEffect {
        effect,
        amplifier: saved.amplifier,
        duration: duration_from_file(saved.duration),
        ambient: saved.ambient,
        show_particles: saved.show_particles,
        show_icon: saved.show_icon.unwrap_or(saved.show_particles),
    }
}

fn active_from_data(effect: MobEffect, data: &EffectData) -> ActiveEffect {
    ActiveEffect {
        effect,
        amplifier: data.amplifier,
        duration: duration_from_file(data.duration),
        ambient: data.ambient,
        show_particles: data.show_particles,
        show_icon: data.show_icon.unwrap_or(data.show_particles),
    }
}

fn duration_from_file(duration: i32) -> u32 {
    if duration < 0 {
        u32::MAX
    } else {
        u32::try_from(duration).unwrap_or(u32::MAX)
    }
}

fn duration_to_file(duration: u32) -> i32 {
    if duration == u32::MAX {
        -1
    } else {
        i32::try_from(duration).unwrap_or(i32::MAX)
    }
}

fn data_from_active(active: &ActiveEffect, hidden: Option<EffectData>) -> EffectData {
    EffectData {
        amplifier: active.amplifier,
        duration: duration_to_file(active.duration),
        ambient: active.ambient,
        show_particles: active.show_particles,
        show_icon: Some(active.show_icon),
        hidden_effect: hidden.map(Box::new),
    }
}

fn is_shorter(shorter: u32, longer: u32) -> bool {
    shorter != u32::MAX && (longer == u32::MAX || shorter < longer)
}

pub(crate) fn merge_hidden(
    previous: Option<ActiveEffect>,
    added: ActiveEffect,
    hidden: &mut PlayerEffectsHidden,
) {
    let Some(previous) = previous else {
        return;
    };
    if added.amplifier > previous.amplifier {
        if is_shorter(added.duration, previous.duration) {
            let nested = hidden.0.remove(&previous.effect);
            hidden
                .0
                .insert(previous.effect, data_from_active(&previous, nested));
        } else {
            hidden.0.remove(&previous.effect);
        }
    } else if is_shorter(previous.duration, added.duration) && previous.amplifier != added.amplifier
    {
        if let Some(existing) = hidden.0.get_mut(&previous.effect) {
            merge_into_data(existing, &added);
        } else {
            hidden
                .0
                .insert(previous.effect, data_from_active(&added, None));
        }
    }
}

fn merge_into_data(existing: &mut EffectData, added: &ActiveEffect) {
    let existing_active = active_from_data(added.effect, existing);
    if added.amplifier > existing_active.amplifier {
        let previous_hidden = existing.hidden_effect.take().map(|nested| *nested);
        let nested = is_shorter(added.duration, existing_active.duration)
            .then(|| data_from_active(&existing_active, previous_hidden));
        *existing = data_from_active(added, nested);
    } else if is_shorter(existing_active.duration, added.duration)
        && existing_active.amplifier != added.amplifier
    {
        if let Some(nested) = &mut existing.hidden_effect {
            merge_into_data(nested, added);
        } else {
            existing.hidden_effect = Some(Box::new(data_from_active(added, None)));
        }
    }
}

pub(crate) fn tick_hidden(hidden: &mut PlayerEffectsHidden) {
    for data in hidden.0.values_mut() {
        tick_effect_data(data);
    }
}

fn tick_effect_data(data: &mut EffectData) {
    if data.duration > 0 {
        data.duration -= 1;
    }
    if let Some(nested) = &mut data.hidden_effect {
        tick_effect_data(nested);
    }
}

pub fn promote_hidden(hidden: &mut PlayerEffectsHidden, effect: MobEffect) -> Option<ActiveEffect> {
    let data = hidden.0.remove(&effect)?;
    if let Some(nested) = data.hidden_effect.clone() {
        hidden.0.insert(effect, *nested);
    }
    Some(active_from_data(effect, &data))
}
