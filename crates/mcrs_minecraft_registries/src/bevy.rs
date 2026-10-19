use crate::Registries;
use bevy_ecs::message::Message;
use bevy_ecs::resource::Resource;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Resource, Clone, Debug)]
pub struct CurrentRegistries(Arc<Registries>);

impl CurrentRegistries {
    pub fn new(registries: Arc<Registries>) -> Self {
        Self(registries)
    }

    pub fn arc(&self) -> &Arc<Registries> {
        &self.0
    }
}

impl Deref for CurrentRegistries {
    type Target = Registries;

    fn deref(&self) -> &Registries {
        &self.0
    }
}

#[derive(Message, Clone, Copy, Debug)]
pub struct RegistriesReloaded;
