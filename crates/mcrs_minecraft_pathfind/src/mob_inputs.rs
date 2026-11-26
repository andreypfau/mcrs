use crate::PathInputs;
use mcrs_minecraft_physics::{Aabb, QueryEntity};

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MobInputs {
    pub entity: QueryEntity,
    pub bounds: Aabb,
    pub on_ground: bool,
    pub in_floatable_fluid: bool,
    pub path: PathInputs,
    pub step_height: f32,
    pub fall_limit: i32,
}
impl MobInputs {
    #[must_use]
    pub fn new(entity: QueryEntity, bounds: Aabb, width: f32, height: f32) -> Self {
        Self {
            path: PathInputs::new(entity.position.into(), width, height),
            entity,
            bounds,
            on_ground: false,
            in_floatable_fluid: false,
            step_height: 0.6,
            fall_limit: 3,
        }
    }
}
