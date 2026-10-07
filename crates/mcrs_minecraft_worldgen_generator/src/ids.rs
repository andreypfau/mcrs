use crate::SurfaceIds;
use crate::multi_noise_biomes::PresetBiomeTables;
use crate::structures::index::EndBiomes;
use bevy_app::{App, Plugin};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::block_tags;
use mcrs_minecraft_registry::shared::Resolved;
use mcrs_minecraft_registry::{LoadReport, RegistrySet, TagId};
use mcrs_minecraft_world::resolvers::AddRegistryResolver;

pub struct GeneratorIdsPlugin;

impl Plugin for GeneratorIdsPlugin {
    fn build(&self, app: &mut App) {
        app.add_registry_resolver(SurfaceIds::resolve)
            .add_registry_resolver(FillIds::resolve)
            .add_registry_resolver(PresetBiomeTables::resolve);
    }
}

pub struct FillIds {
    pub end: EndBiomes,
    pub uncarvable: TagId<Block>,
}

impl FillIds {
    pub fn resolve(set: &RegistrySet, report: &mut LoadReport) -> Option<Resolved<Self>> {
        let biomes = report.registry(set, mcrs_minecraft_biome::keys::BIOME);
        let end = biomes
            .as_ref()
            .and_then(|biomes| EndBiomes::resolve(biomes, report));
        let tags = report.tags(set, mcrs_minecraft_block::keys::BLOCK);
        let uncarvable = tags
            .as_ref()
            .and_then(|tags| report.require_tag(tags, &block_tags::UNCARVABLE));
        Some(Resolved::new(Self {
            end: end?,
            uncarvable: uncarvable?,
        }))
    }
}
