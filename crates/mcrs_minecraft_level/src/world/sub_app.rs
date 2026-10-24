use std::collections::HashSet;

use bevy_app::{App, AppLabel, Plugin, SubApp};
use bevy_ecs::prelude::{Entity, Resource};

use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::{Dimension, DimensionType};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct DimAppLabel(pub Entity);

#[derive(Debug, Clone)]
pub struct DimSpawnRequest {
    pub dimension: ResourceKey<mcrs_minecraft_dimension::Dimension>,
    pub dimension_type: Id<DimensionType>,
}

#[derive(Resource, Default)]
pub struct DimSpawnQueue(pub Vec<DimSpawnRequest>);

/// Queue of host-world `DimSubAppHandle` label entities awaiting sub-app teardown.
/// Entries are the `Entity` values used as `DimAppLabel(Entity)` keys when the
/// sub-app was inserted — **not** the `Dimension` entity that lives inside the
/// sub-app's `World`. The outer runner loop drains this queue and calls
/// `App::remove_sub_app(DimAppLabel(entity))` on each, then despawns the
/// host-side handle entity.
#[derive(Resource, Default)]
pub struct DimDespawnQueue(pub Vec<Entity>);

/// Plugins that the world of every dimension, or of one dimension by its key, adds after its own
/// plugins and resources and before its dimension entity spawns. [`AddDimensionPlugins`] fills it
/// before the dimensions spawn; a dimension that is already live does not get a later plugin. Each
/// world builds its own clone of a plugin, in the order the plugins were added.
///
/// A unique plugin for one dimension replaces the unique plugins of the same name for every
/// dimension in that world. A non-unique plugin for one dimension adds to them. A unique plugin
/// whose name the world already has is skipped with an error.
///
/// A dimension world does not run the stock `Main`. It runs the startup schedules on its first
/// tick, then `First`, `PreUpdate`, the `Fixed*` schedules, `Update`, `PostUpdate` and `Last` once
/// per host tick. It has no `StateTransition` or `TimePlugin`, and its time resources are copies of
/// the host's.
#[derive(Resource, Default)]
pub struct DimensionPlugins(Vec<DimensionPluginEntry>);

struct DimensionPluginEntry {
    only: Option<ResourceKey<Dimension>>,
    name: String,
    unique: bool,
    // Bevy rejects a duplicate by `Plugin::name`, but `SubApp::is_plugin_added` looks up the
    // type name, which a plugin with its own name does not register.
    added: fn(&SubApp, &str) -> bool,
    add: Box<dyn Fn(&mut SubApp) + Send + Sync>,
}

impl DimensionPlugins {
    /// The key of the dimension of each plugin that is added to one dimension alone.
    pub fn dimensions(&self) -> impl Iterator<Item = &ResourceKey<Dimension>> {
        self.0.iter().filter_map(|plugin| plugin.only.as_ref())
    }

    pub fn build(&self, dimension: &ResourceKey<Dimension>, sub_app: &mut SubApp) {
        let replaced: HashSet<&str> = self
            .0
            .iter()
            .filter(|plugin| plugin.unique && plugin.only.as_ref() == Some(dimension))
            .map(|plugin| plugin.name.as_str())
            .collect();
        let mut built = HashSet::new();
        for plugin in &self.0 {
            match &plugin.only {
                Some(only) if only != dimension => continue,
                None if plugin.unique && replaced.contains(plugin.name.as_str()) => continue,
                _ => {}
            }
            if plugin.unique
                && (!built.insert(plugin.name.as_str()) || (plugin.added)(sub_app, &plugin.name))
            {
                tracing::error!(
                    %dimension,
                    plugin = plugin.name,
                    "the dimension world already has this plugin; it is not added again"
                );
                continue;
            }
            (plugin.add)(sub_app);
        }
    }

    fn push<P: Plugin + Clone>(&mut self, only: Option<ResourceKey<Dimension>>, plugin: P) {
        self.0.push(DimensionPluginEntry {
            only,
            name: plugin.name().to_owned(),
            unique: plugin.is_unique(),
            added: |sub_app, name| {
                sub_app
                    .get_added_plugins::<P>()
                    .iter()
                    .any(|plugin| plugin.name() == name)
            },
            add: Box::new(move |sub_app| {
                sub_app.add_plugins(plugin.clone());
            }),
        });
    }
}

pub trait AddDimensionPlugins {
    fn add_dimension_plugin(&mut self, plugin: impl Plugin + Clone) -> &mut Self;

    fn add_dimension_plugin_to(
        &mut self,
        dimension: impl Into<ResourceKey<Dimension>>,
        plugin: impl Plugin + Clone,
    ) -> &mut Self;
}

impl AddDimensionPlugins for App {
    fn add_dimension_plugin(&mut self, plugin: impl Plugin + Clone) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<DimensionPlugins>()
            .push(None, plugin);
        self
    }

    fn add_dimension_plugin_to(
        &mut self,
        dimension: impl Into<ResourceKey<Dimension>>,
        plugin: impl Plugin + Clone,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<DimensionPlugins>()
            .push(Some(dimension.into()), plugin);
        self
    }
}
