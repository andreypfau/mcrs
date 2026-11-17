use mcrs_minecraft_pose::Pose;
use serde::Deserialize;

use crate::entity_view::EntityView;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct EntityFlagsPredicate {
    is_on_ground: Option<bool>,
    is_on_fire: Option<bool>,
    is_sneaking: Option<bool>,
    is_sprinting: Option<bool>,
    is_swimming: Option<bool>,
    is_flying: Option<bool>,
    is_baby: Option<bool>,
    is_in_water: Option<bool>,
    is_fall_flying: Option<bool>,
}

impl EntityFlagsPredicate {
    pub(crate) fn matches<W: EntityView>(&self, world: &W, entity: W::Entity) -> bool {
        let equals = |expected: Option<bool>, actual: fn(&W, W::Entity) -> Option<bool>| {
            expected.is_none_or(|expected| actual(world, entity) == Some(expected))
        };
        let common = equals(self.is_on_ground, W::is_on_ground)
            && equals(self.is_on_fire, W::is_burning)
            && self.is_sneaking.is_none_or(|expected| {
                world
                    .pose(entity)
                    .is_some_and(|pose| (pose == Pose::Crouching) == expected)
            })
            && equals(self.is_sprinting, W::is_sprinting)
            && equals(self.is_swimming, W::is_swimming)
            && equals(self.is_in_water, W::is_in_water);
        if !common {
            return false;
        }
        if self.is_flying.is_none() && self.is_baby.is_none() && self.is_fall_flying.is_none() {
            return true;
        }
        match world.is_living(entity) {
            None => false,
            Some(false) => self.is_flying.is_none_or(|expected| !expected),
            Some(true) => {
                equals(self.is_baby, W::is_baby)
                    && equals(self.is_fall_flying, W::is_fall_flying)
                    && self
                        .is_flying
                        .is_none_or(|expected| is_flying(world, entity) == Some(expected))
            }
        }
    }
}

fn is_flying<W: EntityView>(world: &W, entity: W::Entity) -> Option<bool> {
    match world.is_fall_flying(entity)? {
        true => Some(true),
        false => world.has_flying_ability(entity),
    }
}
