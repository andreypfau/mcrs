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
use mcrs_minecraft_registry::dispatched_map;

dispatched_map! {
    /// The parts an entity must match, keyed by `entity_sub_predicate_type`.
    EntityPredicate on crate::keys::EntitySubPredicateType {
        EntityType => entity_type: HolderSet<EntityType>,
        Location => location: LocationPredicate,
        SteppingOn => stepping_on: LocationPredicate,
        MovementAffectedBy => movement_affected_by: LocationPredicate,
        Distance => distance: DistancePredicate,
        Movement => movement: MovementPredicate,
        Effects => effects: MobEffectsPredicate,
        Nbt => nbt: NbtPredicate,
        Flags => flags: EntityFlagsPredicate,
        Equipment => equipment: EntityEquipmentPredicate,
        PeriodicTick => periodic_tick: PositiveInt,
        Vehicle => vehicle: Box<EntityPredicate>,
        Passenger => passenger: Box<EntityPredicate>,
        TargetedEntity => targeted_entity: Box<EntityPredicate>,
        Team => team: String,
        Slots => slots: SlotsPredicate,
        Components => components: ComponentMap,
        Predicates => predicates: ComponentPredicates,
        EntityTags => entity_tags: EntityTagPredicate,
        TypeSpecificLightning => lightning: LightningBoltPredicate,
        TypeSpecificFishingHook => fishing_hook: FishingHookPredicate,
        TypeSpecificPlayer => player: Box<PlayerPredicate>,
        TypeSpecificCubeMob => cube_mob: CubeMobPredicate,
        TypeSpecificRaider => raider: RaiderPredicate,
        TypeSpecificSheep => sheep: SheepPredicate,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistancePredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub x: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub y: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub z: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub horizontal: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub absolute: MinMaxBounds<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementPredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub x: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub y: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub z: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub speed: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub horizontal_speed: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub vertical_speed: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub fall_distance: MinMaxBounds<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityFlagsPredicate {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_on_ground: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_on_fire: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_sneaking: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_sprinting: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_swimming: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_flying: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_baby: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub is_in_water: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
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
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub blocks_set_on_fire: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_struck: Option<Box<EntityPredicate>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FishingHookPredicate {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub in_open_water: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CubeMobPredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub size: MinMaxBounds<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RaiderPredicate {
    #[serde(
        default,
        skip_serializing_if = "std::ops::Not::not",
        deserialize_with = "mcrs_minecraft_core::codec::flag"
    )]
    pub has_raid: bool,
    #[serde(
        default,
        skip_serializing_if = "std::ops::Not::not",
        deserialize_with = "mcrs_minecraft_core::codec::flag"
    )]
    pub is_captain: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheepPredicate {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub sheared: Option<bool>,
}
