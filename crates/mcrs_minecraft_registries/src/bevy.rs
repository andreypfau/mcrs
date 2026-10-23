use crate::Registries;
use bevy_ecs::resource::Resource;
use std::ops::Deref;

#[derive(Resource, Clone, Debug)]
pub struct CurrentRegistries(Registries);

impl CurrentRegistries {
    pub fn new(registries: Registries) -> Self {
        Self(registries)
    }
}

impl Deref for CurrentRegistries {
    type Target = Registries;

    fn deref(&self) -> &Registries {
        &self.0
    }
}
