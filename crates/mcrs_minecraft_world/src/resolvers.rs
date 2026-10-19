use bevy_app::App;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_registry::{LoadReport, RegistrySet};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Resource)]
pub struct Resolved<T: Send + Sync + 'static>(Arc<T>);

impl<T: Send + Sync + 'static> Resolved<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(value))
    }
}

impl<T: Send + Sync + 'static> Clone for Resolved<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: Send + Sync + 'static> Deref for Resolved<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

type Insert = Arc<dyn Fn(&mut World) + Send + Sync>;
type Resolver = Box<dyn Fn(&RegistrySet, &mut LoadReport) -> Option<Insert> + Send + Sync>;

#[derive(Resource, Default)]
pub(crate) struct RegistryResolvers(Vec<Resolver>);

#[derive(Clone)]
pub struct ResolvedTables(Vec<Insert>);

impl std::fmt::Debug for ResolvedTables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedTables")
            .field("len", &self.0.len())
            .finish()
    }
}

impl ResolvedTables {
    pub fn insert_into(&self, world: &mut World) {
        for insert in &self.0 {
            insert(world);
        }
    }
}

pub trait AddRegistryResolver {
    fn add_registry_resolver<T: Send + Sync + 'static>(
        &mut self,
        resolve: fn(&RegistrySet, &mut LoadReport) -> Option<T>,
    ) -> &mut Self;
}

impl AddRegistryResolver for App {
    fn add_registry_resolver<T: Send + Sync + 'static>(
        &mut self,
        resolve: fn(&RegistrySet, &mut LoadReport) -> Option<T>,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<RegistryResolvers>()
            .0
            .push(Box::new(move |set, report| {
                resolve(set, report).map(|value| {
                    let table = Arc::new(value);
                    Arc::new(move |world: &mut World| {
                        world.insert_resource(Resolved(Arc::clone(&table)));
                    }) as Insert
                })
            }));
        self
    }
}

/// Every resolver runs even after a miss, so the report names all of them; nothing is inserted
/// unless all resolved.
pub fn run_resolvers(world: &mut World, set: &RegistrySet) -> Result<ResolvedTables, LoadReport> {
    let mut report = LoadReport::new();
    let inserts: Vec<Insert> = world
        .get_resource::<RegistryResolvers>()
        .into_iter()
        .flat_map(|resolvers| &resolvers.0)
        .filter_map(|resolve| resolve(set, &mut report))
        .collect();
    if !report.is_empty() {
        return Err(report);
    }
    let tables = ResolvedTables(inserts);
    tables.insert_into(world);
    Ok(tables)
}
