//! The keys of the player file that the server has no behaviour for. Each has a typed value that
//! the server reads, holds and writes back, so the vanilla game gets back what it saved.

use std::cell::Cell;
use std::collections::BTreeMap;

use bevy_ecs::component::Component;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_int_array;
use mcrs_minecraft_text::Text;
use serde::de::{Error as _, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The root keys that [`PlayerFileState`] claims: the serialized field names of the type.
#[must_use]
pub fn player_state_keys() -> &'static [&'static str] {
    struct FieldNames(Cell<&'static [&'static str]>);

    impl<'de> Deserializer<'de> for &FieldNames {
        type Error = serde::de::value::Error;

        fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Self::Error> {
            Err(Self::Error::custom("only the field names are wanted"))
        }

        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _: &'static str,
            fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Self::Error> {
            self.0.set(fields);
            self.deserialize_any(visitor)
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes
            byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map enum
            identifier ignored_any
        }
    }

    let names = FieldNames(Cell::new(&[]));
    let _ = PlayerFileState::deserialize(&names);
    names.0.get()
}

const MAX_TAGS: usize = 1024;

/// An int array of `N` elements: a UUID (four) or a position (three).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ints<const N: usize>(pub [i32; N]);

impl<const N: usize> Serialize for Ints<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        nbt_int_array(self.0.as_slice(), serializer)
    }
}

impl<'de, const N: usize> Deserialize<'de> for Ints<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = Vec::<i32>::deserialize(deserializer)?;
        let found = values.len();
        values
            .try_into()
            .map(Self)
            .map_err(|_| D::Error::custom(format!("an int array of {N}, found {found}")))
    }
}

/// An entity save compound. Its keys depend on the entity type and no rule states them yet, so
/// the value checks the string `id` and keeps the compound as written.
#[derive(Clone, Debug, PartialEq)]
pub struct EntitySave(NbtCompound);

impl Serialize for EntitySave {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EntitySave {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let compound = NbtCompound::deserialize(deserializer)?;
        if compound.get_string("id").is_none() {
            return Err(D::Error::custom(
                "an entity save compound has no string `id`",
            ));
        }
        Ok(Self(compound))
    }
}

/// A memory type of the `Brain`. A player has none, so no value can exist.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoMemory {}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brain {
    #[serde(default)]
    pub memories: BTreeMap<String, NoMemory>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierOperation {
    #[serde(rename = "add_value")]
    AddValue,
    #[serde(rename = "add_multiplied_base")]
    AddMultipliedBase,
    #[serde(rename = "add_multiplied_total")]
    AddMultipliedTotal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modifier {
    pub id: String,
    pub amount: f64,
    pub operation: ModifierOperation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribute {
    pub id: String,
    #[serde(default)]
    pub base: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
}

fn yes() -> bool {
    true
}

/// The keys of a `hidden_effect`: those of its effect except `id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectData {
    #[serde(default)]
    pub amplifier: u8,
    #[serde(default)]
    pub duration: i32,
    #[serde(default)]
    pub ambient: bool,
    #[serde(default = "yes")]
    pub show_particles: bool,
    /// Absent means the value of `show_particles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_icon: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_effect: Option<Box<EffectData>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub id: String,
    #[serde(default)]
    pub amplifier: u8,
    #[serde(default)]
    pub duration: i32,
    #[serde(default)]
    pub ambient: bool,
    #[serde(default = "yes")]
    pub show_particles: bool,
    /// Absent means the value of `show_particles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_icon: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_effect: Option<Box<EffectData>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocatorBarIcon {
    pub style: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<i32>,
}

impl Default for LocatorBarIcon {
    fn default() -> Self {
        Self {
            style: "minecraft:default".to_owned(),
            color: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalPos {
    pub dimension: String,
    pub pos: Ints<3>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawRespawn")]
pub struct Respawn {
    pub dimension: String,
    pub pos: Ints<3>,
    pub yaw: f32,
    pub pitch: f32,
    pub forced: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRespawn {
    dimension: String,
    pos: Ints<3>,
    yaw: f32,
    pitch: f32,
    #[serde(default)]
    forced: bool,
}

impl TryFrom<RawRespawn> for Respawn {
    type Error = String;

    fn try_from(raw: RawRespawn) -> Result<Self, String> {
        if !(-180.0..=180.0).contains(&raw.yaw) {
            return Err(format!(
                "respawn yaw {} is outside -180 through 180",
                raw.yaw
            ));
        }
        if !(-90.0..=90.0).contains(&raw.pitch) {
            return Err(format!(
                "respawn pitch {} is outside -90 through 90",
                raw.pitch
            ));
        }
        Ok(Self {
            dimension: raw.dimension,
            pos: raw.pos,
            yaw: raw.yaw,
            pitch: raw.pitch,
            forced: raw.forced,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "RawWardenTracker", into = "RawWardenTracker")]
pub struct WardenSpawnTracker {
    pub ticks_since_last_warning: u32,
    pub warning_level: u32,
    pub cooldown_ticks: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawWardenTracker {
    #[serde(default)]
    ticks_since_last_warning: i32,
    #[serde(default)]
    warning_level: i32,
    #[serde(default)]
    cooldown_ticks: i32,
}

impl TryFrom<RawWardenTracker> for WardenSpawnTracker {
    type Error = String;

    fn try_from(raw: RawWardenTracker) -> Result<Self, String> {
        let field = |name: &str, value: i32| {
            u32::try_from(value).map_err(|_| format!("warden_spawn_tracker.{name} is negative"))
        };
        Ok(Self {
            ticks_since_last_warning: field(
                "ticks_since_last_warning",
                raw.ticks_since_last_warning,
            )?,
            warning_level: field("warning_level", raw.warning_level)?,
            cooldown_ticks: field("cooldown_ticks", raw.cooldown_ticks)?,
        })
    }
}

impl From<WardenSpawnTracker> for RawWardenTracker {
    fn from(value: WardenSpawnTracker) -> Self {
        let field = |value: u32| i32::try_from(value).unwrap_or(i32::MAX);
        Self {
            ticks_since_last_warning: field(value.ticks_since_last_warning),
            warning_level: field(value.warning_level),
            cooldown_ticks: field(value.cooldown_ticks),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeBook {
    pub recipes: Vec<String>,
    #[serde(rename = "toBeDisplayed")]
    pub to_be_displayed: Vec<String>,
    #[serde(rename = "isGuiOpen", default)]
    pub gui_open: bool,
    #[serde(rename = "isFilteringCraftable", default)]
    pub filtering_craftable: bool,
    #[serde(rename = "isFurnaceGuiOpen", default)]
    pub furnace_gui_open: bool,
    #[serde(rename = "isFurnaceFilteringCraftable", default)]
    pub furnace_filtering_craftable: bool,
    #[serde(rename = "isBlastingFurnaceGuiOpen", default)]
    pub blasting_furnace_gui_open: bool,
    #[serde(rename = "isBlastingFurnaceFilteringCraftable", default)]
    pub blasting_furnace_filtering_craftable: bool,
    #[serde(rename = "isSmokerGuiOpen", default)]
    pub smoker_gui_open: bool,
    #[serde(rename = "isSmokerFilteringCraftable", default)]
    pub smoker_filtering_craftable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootVehicle {
    #[serde(rename = "Attach")]
    pub attach: Ints<4>,
    #[serde(rename = "Entity")]
    pub entity: EntitySave,
}

/// A byte that is true only when set: written only then.
fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

fn tags_in_range<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let tags = Vec::<String>::deserialize(deserializer)?;
    if tags.len() > MAX_TAGS {
        return Err(D::Error::custom(format!(
            "{} tags, at most {MAX_TAGS}",
            tags.len()
        )));
    }
    Ok(tags)
}

fn is_default_icon(icon: &LocatorBarIcon) -> bool {
    *icon == LocatorBarIcon::default()
}

/// The values of the player file that no other component of the player owns. Every field has the
/// default of the file layout, and the `skip_serializing_if` fields are the ones the file holds
/// only when they are set.
#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerFileState {
    #[serde(rename = "Motion")]
    pub motion: [f64; 3],
    pub fall_distance: f64,
    #[serde(rename = "Fire")]
    pub fire: i16,
    #[serde(rename = "Air")]
    pub air: i16,
    #[serde(rename = "OnGround")]
    pub on_ground: bool,
    #[serde(rename = "Invulnerable")]
    pub invulnerable: bool,
    #[serde(rename = "PortalCooldown")]
    pub portal_cooldown: i32,
    #[serde(skip_serializing_if = "is_zero")]
    pub invulnerable_time: i32,
    #[serde(rename = "CustomName", skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<Text>,
    #[serde(rename = "CustomNameVisible", skip_serializing_if = "is_false")]
    pub custom_name_visible: bool,
    #[serde(rename = "Silent", skip_serializing_if = "is_false")]
    pub silent: bool,
    #[serde(rename = "NoGravity", skip_serializing_if = "is_false")]
    pub no_gravity: bool,
    #[serde(rename = "Glowing", skip_serializing_if = "is_false")]
    pub glowing: bool,
    #[serde(rename = "TicksFrozen", skip_serializing_if = "is_zero")]
    pub ticks_frozen: i32,
    #[serde(rename = "HasVisualFire", skip_serializing_if = "is_false")]
    pub has_visual_fire: bool,
    #[serde(
        rename = "Tags",
        deserialize_with = "tags_in_range",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub tags: Vec<String>,
    /// Custom data: free-form by definition.
    #[serde(skip_serializing_if = "NbtCompound::is_empty")]
    pub data: NbtCompound,
    #[serde(rename = "Passengers", skip_serializing_if = "Vec::is_empty")]
    pub passengers: Vec<EntitySave>,
    /// The game reads it and never writes it.
    #[serde(rename = "Team", skip_serializing)]
    pub team: Option<String>,

    #[serde(rename = "Health")]
    pub health: f32,
    #[serde(rename = "HurtTime")]
    pub hurt_time: i16,
    #[serde(rename = "DeathTime")]
    pub death_time: i16,
    #[serde(rename = "AbsorptionAmount")]
    pub absorption_amount: f32,
    pub attributes: Vec<Attribute>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub active_effects: Vec<Effect>,
    #[serde(rename = "FallFlying")]
    pub fall_flying: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sleeping_pos: Option<Ints<3>>,
    #[serde(rename = "Brain")]
    pub brain: Brain,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_hurt_by_player: Option<Ints<4>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_hurt_by_player_memory_time: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_hurt_by_mob: Option<Ints<4>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticks_since_last_hurt_by_mob: Option<i32>,
    #[serde(skip_serializing_if = "is_default_icon")]
    pub locator_bar_icon: LocatorBarIcon,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_explosion_impact_pos: Option<[f64; 3]>,
    pub current_impulse_context_reset_grace_time: i32,

    #[serde(
        rename = "previousPlayerGameType",
        skip_serializing_if = "Option::is_none"
    )]
    pub previous_player_game_type: Option<i32>,
    #[serde(rename = "foodLevel")]
    pub food_level: i32,
    #[serde(rename = "foodTickTimer")]
    pub food_tick_timer: i32,
    #[serde(rename = "foodSaturationLevel")]
    pub food_saturation_level: f32,
    #[serde(rename = "foodExhaustionLevel")]
    pub food_exhaustion_level: f32,
    #[serde(rename = "XpLevel")]
    pub xp_level: i32,
    #[serde(rename = "XpTotal")]
    pub xp_total: i32,
    #[serde(rename = "Score")]
    pub score: i32,
    #[serde(rename = "XpP")]
    pub xp_progress: f32,
    #[serde(rename = "XpSeed")]
    pub xp_seed: i32,
    #[serde(rename = "SleepTimer")]
    pub sleep_timer: i16,
    #[serde(rename = "LastDeathLocation", skip_serializing_if = "Option::is_none")]
    pub last_death_location: Option<GlobalPos>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub respawn: Option<Respawn>,
    #[serde(rename = "seenCredits")]
    pub seen_credits: bool,
    pub spawn_extra_particles_on_fall: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raid_omen_position: Option<Ints<3>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entered_nether_pos: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_explosion_impact_pos: Option<[f64; 3]>,
    pub warden_spawn_tracker: WardenSpawnTracker,
    #[serde(rename = "recipeBook")]
    pub recipe_book: RecipeBook,
    pub post_effects: Vec<String>,
    #[serde(rename = "ShoulderEntityLeft", skip_serializing_if = "Option::is_none")]
    pub shoulder_entity_left: Option<EntitySave>,
    #[serde(
        rename = "ShoulderEntityRight",
        skip_serializing_if = "Option::is_none"
    )]
    pub shoulder_entity_right: Option<EntitySave>,
    #[serde(rename = "RootVehicle", skip_serializing_if = "Option::is_none")]
    pub root_vehicle: Option<RootVehicle>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ender_pearls: Vec<EntitySave>,
}

impl Default for PlayerFileState {
    fn default() -> Self {
        Self {
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire: 0,
            air: 300,
            on_ground: false,
            invulnerable: false,
            portal_cooldown: 0,
            invulnerable_time: 0,
            custom_name: None,
            custom_name_visible: false,
            silent: false,
            no_gravity: false,
            glowing: false,
            ticks_frozen: 0,
            has_visual_fire: false,
            tags: Vec::new(),
            data: NbtCompound::new(),
            passengers: Vec::new(),
            team: None,
            health: 20.0,
            hurt_time: 0,
            death_time: 0,
            absorption_amount: 0.0,
            attributes: Vec::new(),
            active_effects: Vec::new(),
            fall_flying: false,
            sleeping_pos: None,
            brain: Brain::default(),
            last_hurt_by_player: None,
            last_hurt_by_player_memory_time: None,
            last_hurt_by_mob: None,
            ticks_since_last_hurt_by_mob: None,
            locator_bar_icon: LocatorBarIcon::default(),
            current_explosion_impact_pos: None,
            current_impulse_context_reset_grace_time: 0,
            previous_player_game_type: None,
            food_level: 20,
            food_tick_timer: 0,
            food_saturation_level: 5.0,
            food_exhaustion_level: 0.0,
            xp_level: 0,
            xp_total: 0,
            score: 0,
            xp_progress: 0.0,
            xp_seed: 0,
            sleep_timer: 0,
            last_death_location: None,
            respawn: None,
            seen_credits: false,
            spawn_extra_particles_on_fall: false,
            raid_omen_position: None,
            entered_nether_pos: None,
            last_explosion_impact_pos: None,
            warden_spawn_tracker: WardenSpawnTracker::default(),
            recipe_book: RecipeBook::default(),
            post_effects: Vec::new(),
            shoulder_entity_left: None,
            shoulder_entity_right: None,
            root_vehicle: None,
            ender_pearls: Vec::new(),
        }
    }
}
