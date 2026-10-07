use bevy_app::App;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_registry::shared::{SharedResource, share};
use mcrs_minecraft_registry::{LoadReport, RegistrySet};

type Insert = Box<dyn FnOnce(&mut World)>;
type Resolver = Box<dyn Fn(&RegistrySet, &mut LoadReport) -> Option<Insert> + Send + Sync>;

#[derive(Resource, Default)]
pub(crate) struct RegistryResolvers(Vec<Resolver>);

pub trait AddRegistryResolver {
    fn add_registry_resolver<T: SharedResource>(
        &mut self,
        resolve: fn(&RegistrySet, &mut LoadReport) -> Option<T>,
    ) -> &mut Self;
}

impl AddRegistryResolver for App {
    fn add_registry_resolver<T: SharedResource>(
        &mut self,
        resolve: fn(&RegistrySet, &mut LoadReport) -> Option<T>,
    ) -> &mut Self {
        share::<T>(self.world_mut());
        self.world_mut()
            .get_resource_or_init::<RegistryResolvers>()
            .0
            .push(Box::new(move |set, report| {
                resolve(set, report).map(|value| {
                    Box::new(move |world: &mut World| world.insert_resource(value)) as Insert
                })
            }));
        self
    }
}

/// Every resolver runs even after a miss, so the report names all of them; nothing is inserted
/// unless all resolved.
pub fn run_resolvers(world: &mut World, set: &RegistrySet) -> Result<(), LoadReport> {
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
    for insert in inserts {
        insert(world);
    }
    Ok(())
}
