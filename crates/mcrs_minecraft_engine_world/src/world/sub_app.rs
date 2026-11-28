use std::collections::HashSet;

use bevy_app::{App, Plugin, SubApp};
use bevy_ecs::prelude::{Entity, Resource};
use bevy_ecs::schedule::SystemSet;

use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimensionLabel(pub Entity);

#[derive(Debug, Clone)]
pub struct DimSpawnRequest {
    pub dimension: ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    pub dimension_type: Id<DimensionType>,
}

#[derive(Resource, Default)]
pub struct DimSpawnQueue(pub Vec<DimSpawnRequest>);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DimSpawnEnqueue;

#[derive(Resource, Default)]
pub struct DimDespawnQueue(pub Vec<Entity>);

#[derive(Resource, Default)]
pub struct DimensionPlugins(Vec<DimensionPluginEntry>);

struct DimensionPluginEntry {
    only: Option<ResourceKeyBuf<Dimension>>,
    name: String,
    unique: bool,
    added: fn(&SubApp, &str) -> bool,
    add: Box<dyn Fn(&mut SubApp) + Send + Sync>,
}

impl DimensionPlugins {
    pub fn dimensions(&self) -> impl Iterator<Item = &ResourceKeyBuf<Dimension>> {
        self.0.iter().filter_map(|plugin| plugin.only.as_ref())
    }

    pub fn build(&self, dimension: &ResourceKeyBuf<Dimension>, sub_app: &mut SubApp) {
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

    fn push<P: Plugin + Clone>(&mut self, only: Option<ResourceKeyBuf<Dimension>>, plugin: P) {
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
        dimension: impl Into<ResourceKeyBuf<Dimension>>,
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
        dimension: impl Into<ResourceKeyBuf<Dimension>>,
        plugin: impl Plugin + Clone,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<DimensionPlugins>()
            .push(Some(dimension.into()), plugin);
        self
    }
}
