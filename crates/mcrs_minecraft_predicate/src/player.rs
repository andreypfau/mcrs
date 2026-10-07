use std::collections::BTreeMap;

use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_item::component::common::MinMaxBounds;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_item::recipe::Recipe;
use mcrs_minecraft_protocol::GameMode;
use serde::{Deserialize, Serialize};

use crate::entity::EntityPredicate;
use crate::keys::CustomStat;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerPredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub level: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "FoodPredicate::is_any")]
    pub food: FoodPredicate,
    #[serde(default, skip_serializing_if = "GameModes::is_any")]
    pub gamemode: GameModes,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stats: Vec<StatMatcher>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub recipes: BTreeMap<ResourceKey<Recipe>, bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub advancements: BTreeMap<ResourceLocation, AdvancementPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub looking_at: Option<Box<EntityPredicate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<InputPredicate>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoodPredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub level: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub saturation: MinMaxBounds<f64>,
}

impl FoodPredicate {
    pub fn is_any(&self) -> bool {
        self.level.is_any() && self.saturation.is_any()
    }
}

/// The game modes a player may be in; every mode, in order, is no constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameModes(pub Vec<GameMode>);

impl GameModes {
    const ANY: [GameMode; 4] = [
        GameMode::Survival,
        GameMode::Creative,
        GameMode::Adventure,
        GameMode::Spectator,
    ];

    pub fn is_any(&self) -> bool {
        self.0 == Self::ANY
    }
}

impl Default for GameModes {
    fn default() -> Self {
        GameModes(Self::ANY.to_vec())
    }
}

/// A statistic and the range its value must fall in, dispatched on the
/// statistic's type, which names the registry the statistic is read from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum StatMatcher {
    Mined(StatOf<Block>),
    Crafted(StatOf<Item>),
    Used(StatOf<Item>),
    Broken(StatOf<Item>),
    PickedUp(StatOf<Item>),
    Dropped(StatOf<Item>),
    Killed(StatOf<EntityType>),
    KilledBy(StatOf<EntityType>),
    Custom(StatOf<CustomStat>),
}

mcrs_minecraft_registry::dispatch! {
    StatMatcher, key = "type", registry = crate::keys::StatType,
    {
        Mined => Mined,
        Crafted => Crafted,
        Used => Used,
        Broken => Broken,
        PickedUp => PickedUp,
        Dropped => Dropped,
        Killed => Killed,
        KilledBy => KilledBy,
        Custom => Custom,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatOf<T> {
    pub stat: T,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub value: MinMaxBounds<i32>,
}

/// An advancement done or not, or the criteria of it that must be done or not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AdvancementPredicate {
    Done(bool),
    Criteria(BTreeMap<String, bool>),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputPredicate {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub forward: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub backward: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub left: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub right: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub jump: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub sneak: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub sprint: Option<bool>,
}
