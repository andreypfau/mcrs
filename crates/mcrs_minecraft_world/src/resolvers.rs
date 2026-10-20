use bevy_app::App;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_registries::{Diagnostic, Diagnostics, Level, Registries};
use std::any::type_name;
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
type Resolver = Box<dyn Fn(&Registries, &mut Diagnostics) -> Option<Insert> + Send + Sync>;

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
        resolve: fn(&Registries, &mut Diagnostics) -> Option<T>,
    ) -> &mut Self;
}

fn is_error(diagnostic: &Diagnostic) -> bool {
    diagnostic.level() == Level::Error
}

impl AddRegistryResolver for App {
    fn add_registry_resolver<T: Send + Sync + 'static>(
        &mut self,
        resolve: fn(&Registries, &mut Diagnostics) -> Option<T>,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<RegistryResolvers>()
            .0
            .push(Box::new(move |registries, diagnostics| {
                let before = diagnostics.len();
                let value = resolve(registries, diagnostics);
                if value.is_none() && !diagnostics[before..].iter().any(is_error) {
                    diagnostics.extend([Diagnostic::Validation {
                        registry: rl!("mcrs:resolvers").into(),
                        message: format!("{} did not resolve", type_name::<T>()),
                    }]);
                }
                value.map(|value| {
                    let table = Arc::new(value);
                    Arc::new(move |world: &mut World| {
                        world.insert_resource(Resolved(Arc::clone(&table)));
                    }) as Insert
                })
            }));
        self
    }
}

/// Every resolver runs even after a miss, so the diagnostics name all of them; nothing is inserted
/// unless all resolved. The diagnostics are sorted and deduplicated by their rendered line, so they
/// do not depend on registration order.
pub fn run_resolvers(
    world: &mut World,
    registries: &Registries,
) -> Result<ResolvedTables, Diagnostics> {
    let mut collected = Diagnostics::default();
    let inserts: Vec<Option<Insert>> = world
        .get_resource::<RegistryResolvers>()
        .into_iter()
        .flat_map(|resolvers| &resolvers.0)
        .map(|resolve| resolve(registries, &mut collected))
        .collect();
    if collected.iter().any(is_error) || inserts.iter().any(Option::is_none) {
        let mut lines: Vec<(String, Diagnostic)> = collected
            .iter()
            .map(|diagnostic| (diagnostic.to_string(), diagnostic.clone()))
            .collect();
        lines.sort_by(|a, b| a.0.cmp(&b.0));
        lines.dedup_by(|a, b| a.0 == b.0);
        let mut sorted = Diagnostics::default();
        sorted.extend(lines.into_iter().map(|(_, diagnostic)| diagnostic));
        return Err(sorted);
    }
    let tables = ResolvedTables(inserts.into_iter().flatten().collect());
    tables.insert_into(world);
    Ok(tables)
}
