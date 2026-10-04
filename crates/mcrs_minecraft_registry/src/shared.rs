use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use std::any::{TypeId, type_name};
use std::fmt;

pub trait SharedResource: Resource + Clone {
    fn shares_with(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingShared {
    pub type_name: &'static str,
}

impl fmt::Display for MissingShared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "shared resource {} is missing from the host world",
            self.type_name
        )
    }
}

impl std::error::Error for MissingShared {}

struct Entry {
    id: TypeId,
    type_name: &'static str,
    copy: fn(&World, &mut World) -> Result<(), MissingShared>,
    shares: fn(&World, &World) -> Option<bool>,
}

#[derive(Resource, Default)]
pub struct SharedRegistries {
    entries: Vec<Entry>,
}

impl SharedRegistries {
    pub fn copy_into(&self, host: &World, dim: &mut World) -> Result<(), MissingShared> {
        self.entries
            .iter()
            .try_for_each(|entry| (entry.copy)(host, dim))
    }

    pub fn shared_in(&self, host: &World, dim: &World) -> Vec<(&'static str, Option<bool>)> {
        self.entries
            .iter()
            .map(|entry| (entry.type_name, (entry.shares)(host, dim)))
            .collect()
    }
}

pub fn share<T: SharedResource>(world: &mut World) {
    world.init_resource::<SharedRegistries>();
    let mut shared = world.resource_mut::<SharedRegistries>();
    let id = TypeId::of::<T>();
    if shared.entries.iter().any(|entry| entry.id == id) {
        return;
    }
    shared.entries.push(Entry {
        id,
        type_name: type_name::<T>(),
        copy: |host, dim| {
            let value = host.get_resource::<T>().ok_or(MissingShared {
                type_name: type_name::<T>(),
            })?;
            dim.insert_resource(value.clone());
            Ok(())
        },
        shares: |host, dim| {
            Some(
                host.get_resource::<T>()?
                    .shares_with(dim.get_resource::<T>()?),
            )
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Resource, Clone)]
    struct Table(Arc<Vec<u32>>);

    impl SharedResource for Table {
        fn shares_with(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }

    #[test]
    fn a_shared_resource_the_host_lacks_is_named() {
        let mut host = World::new();
        share::<Table>(&mut host);
        let mut dim = World::new();
        let error = host
            .resource::<SharedRegistries>()
            .copy_into(&host, &mut dim)
            .unwrap_err();
        assert!(error.to_string().contains("Table"), "{error}");
    }

    #[test]
    fn a_type_shared_twice_is_registered_once() {
        let mut host = World::new();
        host.insert_resource(Table(Arc::new(vec![1])));
        share::<Table>(&mut host);
        share::<Table>(&mut host);
        let mut dim = World::new();
        let shared = host.resource::<SharedRegistries>();
        shared.copy_into(&host, &mut dim).unwrap();
        let seen = shared.shared_in(&host, &dim);
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(seen[0].1, Some(true));
    }
}
