use std::collections::BTreeMap;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_registries::{Named, Refs, Resolve, Resolved, registry_value};
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use serde::{Deserialize, Deserializer};

use crate::custom_stat_keys::CustomStat;
use crate::entity_predicate::EntityPredicate;
use crate::stat_type_keys::StatType;

#[registry_value]
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub(crate) struct PlayerPredicate<M: Refs = Resolved> {
    #[serde(default)]
    level: MinMaxBounds<i32>,
    #[serde(default)]
    food: FoodPredicate,
    #[serde(default = "every_game_mode")]
    gamemode: Vec<GameMode>,
    #[serde(default)]
    stats: Vec<StatPredicate<M>>,
    #[serde(default)]
    recipes: BTreeMap<ResourceLocationBuf, bool>,
    #[serde(default)]
    advancements: BTreeMap<ResourceLocationBuf, AdvancementPredicate>,
    looking_at: Option<Box<EntityPredicate<M>>>,
    input: Option<InputPredicate>,
}

impl PlayerPredicate {
    pub(crate) fn tests_only_the_type(&self) -> bool {
        self.level.is_any()
            && self.food.level.is_any()
            && self.food.saturation.is_any()
            && self.gamemode == GameMode::ALL
            && self.stats.is_empty()
            && self.recipes.is_empty()
            && self.advancements.is_empty()
            && self.looking_at.is_none()
            && self.input.is_none()
    }

    pub(crate) fn looking_at(&self) -> Option<&EntityPredicate> {
        self.looking_at.as_deref()
    }
}

fn every_game_mode() -> Vec<GameMode> {
    GameMode::ALL.to_vec()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub(crate) struct FoodPredicate {
    #[serde(default)]
    level: MinMaxBounds<i32>,
    #[serde(default)]
    saturation: MinMaxBounds<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum AdvancementPredicate {
    Done(bool),
    Criteria(BTreeMap<String, bool>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct InputPredicate {
    forward: Option<bool>,
    backward: Option<bool>,
    left: Option<bool>,
    right: Option<bool>,
    jump: Option<bool>,
    sneak: Option<bool>,
    sprint: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) struct StatPredicate<M: Refs = Resolved> {
    stat: Stat<M>,
    value: MinMaxBounds<i32>,
}

#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) enum Stat<M: Refs = Resolved> {
    Mined(M::Id<Block>),
    Crafted(M::Id<Item>),
    Used(M::Id<Item>),
    Broken(M::Id<Item>),
    PickedUp(M::Id<Item>),
    Dropped(M::Id<Item>),
    Killed(M::Id<EntityType>),
    KilledBy(M::Id<EntityType>),
    Custom(M::Id<CustomStat>),
}

impl<'de> Deserialize<'de> for StatPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            #[serde(rename = "type")]
            stat_type: StatType,
            stat: ResourceLocationBuf,
            #[serde(default)]
            value: MinMaxBounds<i32>,
        }

        let repr = Repr::deserialize(d)?;
        let entry = repr.stat;
        let stat = match repr.stat_type {
            StatType::Mined => Stat::Mined(ResourceKeyBuf::from_location(entry)),
            StatType::Crafted => Stat::Crafted(ResourceKeyBuf::from_location(entry)),
            StatType::Used => Stat::Used(ResourceKeyBuf::from_location(entry)),
            StatType::Broken => Stat::Broken(ResourceKeyBuf::from_location(entry)),
            StatType::PickedUp => Stat::PickedUp(ResourceKeyBuf::from_location(entry)),
            StatType::Dropped => Stat::Dropped(ResourceKeyBuf::from_location(entry)),
            StatType::Killed => Stat::Killed(ResourceKeyBuf::from_location(entry)),
            StatType::KilledBy => Stat::KilledBy(ResourceKeyBuf::from_location(entry)),
            StatType::Custom => Stat::Custom(ResourceKeyBuf::from_location(entry)),
        };
        Ok(StatPredicate {
            stat,
            value: repr.value,
        })
    }
}
