use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use std::sync::Arc;

#[derive(Resource)]
pub struct Versioned<T: Send + Sync + 'static> {
    value: Arc<T>,
    version: u64,
}

impl<T: Send + Sync + 'static> Versioned<T> {
    pub fn new(value: Arc<T>) -> Self {
        Self { value, version: 0 }
    }

    pub fn value(&self) -> &Arc<T> {
        &self.value
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn replace(&mut self, value: Arc<T>) {
        self.value = value;
        self.version = self.version.wrapping_add(1);
    }

    pub fn extract(main: &World, sub: &mut World) -> Option<Arc<T>> {
        let source = main.get_resource::<Self>()?;
        if sub
            .get_resource::<Self>()
            .is_some_and(|held| held.version == source.version)
        {
            return None;
        }
        let value = Arc::clone(&source.value);
        sub.insert_resource(source.clone());
        Some(value)
    }
}

impl<T: Send + Sync + 'static> Clone for Versioned<T> {
    fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
            version: self.version,
        }
    }
}
