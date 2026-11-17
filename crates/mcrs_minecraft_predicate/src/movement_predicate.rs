use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use serde::Deserialize;

use crate::distance_predicate::matches_squared;
use crate::entity_view::EntityView;

const TICKS_PER_SECOND: f64 = 20.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub(crate) struct MovementPredicate {
    #[serde(default)]
    x: MinMaxBounds<f64>,
    #[serde(default)]
    y: MinMaxBounds<f64>,
    #[serde(default)]
    z: MinMaxBounds<f64>,
    #[serde(default)]
    speed: MinMaxBounds<f64>,
    #[serde(default)]
    horizontal_speed: MinMaxBounds<f64>,
    #[serde(default)]
    vertical_speed: MinMaxBounds<f64>,
    #[serde(default)]
    fall_distance: MinMaxBounds<f64>,
}

impl MovementPredicate {
    pub(crate) fn matches<W: EntityView>(&self, world: &W, entity: W::Entity) -> bool {
        let velocity_tested = !(self.x.is_any()
            && self.y.is_any()
            && self.z.is_any()
            && self.speed.is_any()
            && self.horizontal_speed.is_any()
            && self.vertical_speed.is_any());
        if velocity_tested {
            let Some(movement) = world.reported_movement(entity) else {
                return false;
            };
            let velocity = movement * TICKS_PER_SECOND;
            let horizontal = velocity.x * velocity.x + velocity.z * velocity.z;
            let passes = self.x.matches(velocity.x)
                && self.y.matches(velocity.y)
                && self.z.matches(velocity.z)
                && matches_squared(&self.speed, horizontal + velocity.y * velocity.y)
                && matches_squared(&self.horizontal_speed, horizontal)
                && self.vertical_speed.matches(velocity.y.abs());
            if !passes {
                return false;
            }
        }
        self.fall_distance.is_any()
            || world
                .fall_distance(entity)
                .is_some_and(|fall| self.fall_distance.matches(fall))
    }
}
