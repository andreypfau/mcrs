use mcrs_minecraft_core::codec::PositiveInt;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_item::component::common::{MinMaxBounds, NbtPredicate};
use mcrs_minecraft_item::component::predicate::{
    ComponentPredicates, ItemPredicate, MobEffectsPredicate,
};
use mcrs_minecraft_item::patch::ComponentMap;
use mcrs_minecraft_registry::HolderSet;
use serde::{Deserialize, Serialize};

use crate::location::LocationPredicate;
use crate::player::PlayerPredicate;
use crate::slots::SlotsPredicate;
use crate::{dispatched_map, is_any_double, is_any_int};

dispatched_map! {
    /// The parts an entity must match, keyed by `entity_sub_predicate_type`.
    EntityPredicate {
        "minecraft:entity_type" => entity_type: HolderSet<EntityType>,
        "minecraft:location" => location: LocationPredicate,
        "minecraft:stepping_on" => stepping_on: LocationPredicate,
        "minecraft:movement_affected_by" => movement_affected_by: LocationPredicate,
        "minecraft:distance" => distance: DistancePredicate,
        "minecraft:movement" => movement: MovementPredicate,
        "minecraft:effects" => effects: MobEffectsPredicate,
        "minecraft:nbt" => nbt: NbtPredicate,
        "minecraft:flags" => flags: EntityFlagsPredicate,
        "minecraft:equipment" => equipment: EntityEquipmentPredicate,
        "minecraft:periodic_tick" => periodic_tick: PositiveInt,
        "minecraft:vehicle" => vehicle: Box<EntityPredicate>,
        "minecraft:passenger" => passenger: Box<EntityPredicate>,
        "minecraft:targeted_entity" => targeted_entity: Box<EntityPredicate>,
        "minecraft:team" => team: String,
        "minecraft:slots" => slots: SlotsPredicate,
        "minecraft:components" => components: ComponentMap,
        "minecraft:predicates" => predicates: ComponentPredicates,
        "minecraft:entity_tags" => entity_tags: EntityTagPredicate,
        "minecraft:type_specific/lightning" => lightning: LightningBoltPredicate,
        "minecraft:type_specific/fishing_hook" => fishing_hook: FishingHookPredicate,
        "minecraft:type_specific/player" => player: Box<PlayerPredicate>,
        "minecraft:type_specific/cube_mob" => cube_mob: CubeMobPredicate,
        "minecraft:type_specific/raider" => raider: RaiderPredicate,
        "minecraft:type_specific/sheep" => sheep: SheepPredicate,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistancePredicate {
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub x: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub y: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub z: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub horizontal: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub absolute: MinMaxBounds<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementPredicate {
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub x: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub y: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub z: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub speed: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub horizontal_speed: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub vertical_speed: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "is_any_double")]
    pub fall_distance: MinMaxBounds<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityFlagsPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_on_ground: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_on_fire: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_sneaking: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_sprinting: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_swimming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_flying: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_baby: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_in_water: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_fall_flying: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityEquipmentPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<ItemPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chest: Option<ItemPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<ItemPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feet: Option<ItemPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<ItemPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mainhand: Option<ItemPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offhand: Option<ItemPredicate>,
}

/// Scoreboard tags the entity carries, none of which name a registry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityTagPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub any_of: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_of: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub none_of: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightningBoltPredicate {
    #[serde(default, skip_serializing_if = "is_any_int")]
    pub blocks_set_on_fire: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_struck: Option<Box<EntityPredicate>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FishingHookPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_open_water: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CubeMobPredicate {
    #[serde(default, skip_serializing_if = "is_any_int")]
    pub size: MinMaxBounds<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RaiderPredicate {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_raid: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_captain: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheepPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheared: Option<bool>,
}
