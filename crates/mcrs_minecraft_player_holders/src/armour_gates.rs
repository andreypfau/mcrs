use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_item::Gate;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmourGates {
    pub(crate) place: Gate,
    pub(crate) take: Gate,
}

impl ArmourGates {
    pub(crate) const REFUSE: Self = Self {
        place: Gate::Refuses,
        take: Gate::Refuses,
    };
}
