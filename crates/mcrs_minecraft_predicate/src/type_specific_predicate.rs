use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_registries::{Refs, Resolve, Resolved, registry_value};
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use serde::Deserialize;

use crate::entity_predicate::EntityPredicate;
use crate::player_predicate::PlayerPredicate;

#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) enum TypeSpecificPredicate<M: Refs = Resolved> {
    Lightning(LightningPredicate<M>),
    FishingHook(FishingHookPredicate),
    Player(Box<PlayerPredicate<M>>),
    CubeMob(CubeMobPredicate),
    Raider(RaiderPredicate),
    Sheep(SheepPredicate),
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub(crate) struct LightningPredicate<M: Refs = Resolved> {
    #[serde(default)]
    blocks_set_on_fire: MinMaxBounds<i32>,
    entity_struck: Option<Box<EntityPredicate<M>>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct FishingHookPredicate {
    in_open_water: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub(crate) struct CubeMobPredicate {
    #[serde(default)]
    size: MinMaxBounds<i32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct RaiderPredicate {
    #[serde(default)]
    has_raid: bool,
    #[serde(default)]
    is_captain: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct SheepPredicate {
    sheared: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeSpecificScope {
    Every,
    EntityTypes(&'static [EntityType]),
    Unmatched,
}

impl TypeSpecificPredicate {
    pub(crate) fn scope(&self) -> TypeSpecificScope {
        match self {
            TypeSpecificPredicate::Lightning(lightning)
                if lightning.blocks_set_on_fire.is_any() && lightning.entity_struck.is_none() =>
            {
                TypeSpecificScope::EntityTypes(&[EntityType::LightningBolt])
            }
            TypeSpecificPredicate::FishingHook(FishingHookPredicate {
                in_open_water: None,
            }) => TypeSpecificScope::Every,
            TypeSpecificPredicate::Player(player) if player.tests_only_the_type() => {
                TypeSpecificScope::EntityTypes(&[EntityType::Player])
            }
            TypeSpecificPredicate::CubeMob(cube_mob) if cube_mob.size.is_any() => {
                TypeSpecificScope::EntityTypes(&[
                    EntityType::Slime,
                    EntityType::MagmaCube,
                    EntityType::SulfurCube,
                ])
            }
            TypeSpecificPredicate::Sheep(SheepPredicate { sheared: None }) => {
                TypeSpecificScope::EntityTypes(&[EntityType::Sheep])
            }
            TypeSpecificPredicate::Lightning(_)
            | TypeSpecificPredicate::FishingHook(_)
            | TypeSpecificPredicate::Player(_)
            | TypeSpecificPredicate::CubeMob(_)
            | TypeSpecificPredicate::Raider(_)
            | TypeSpecificPredicate::Sheep(_) => TypeSpecificScope::Unmatched,
        }
    }

    pub(crate) fn nested(&self) -> Option<&EntityPredicate> {
        match self {
            TypeSpecificPredicate::Lightning(lightning) => lightning.entity_struck.as_deref(),
            TypeSpecificPredicate::Player(player) => player.looking_at(),
            TypeSpecificPredicate::FishingHook(_)
            | TypeSpecificPredicate::CubeMob(_)
            | TypeSpecificPredicate::Raider(_)
            | TypeSpecificPredicate::Sheep(_) => None,
        }
    }
}
