use crate::SurfaceIds;
use crate::structures::index::EndBiomes;
use bevy_app::{App, Plugin};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::Fluid;
use mcrs_minecraft_block::keys::block_tags;
use mcrs_minecraft_block::keys::fluid_tags;
use mcrs_minecraft_core::TagKey;
use mcrs_minecraft_registry::shared::Resolved;
use mcrs_minecraft_registry::{LoadReport, RegistrySet, TagId};
use mcrs_minecraft_world::resolvers::AddRegistryResolver;

pub struct GeneratorIdsPlugin;

impl Plugin for GeneratorIdsPlugin {
    fn build(&self, app: &mut App) {
        app.add_registry_resolver(SurfaceIds::resolve)
            .add_registry_resolver(FillIds::resolve)
            .add_registry_resolver(SurvivalIds::resolve);
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

pub struct SurvivalIds {
    pub overrides_mushroom_light_requirement: TagId<Block>,
    pub cannot_support_seagrass: TagId<Block>,
    pub supports_small_dripleaf: TagId<Block>,
    pub supports_vegetation: TagId<Block>,
    pub unstable_bottom_center: TagId<Block>,
    pub supports_lily_pad: TagId<Block>,
    pub supports_lily_pad_fluids: TagId<Fluid>,
    pub supports_sugar_cane: TagId<Block>,
    pub supports_sugar_cane_adjacently: TagId<Block>,
    pub supports_cactus: TagId<Block>,
}

impl SurvivalIds {
    pub fn resolve(set: &RegistrySet, report: &mut LoadReport) -> Option<Resolved<Self>> {
        let blocks = report.tags(set, mcrs_minecraft_block::keys::BLOCK);
        let fluids = report.tags(set, mcrs_minecraft_block::keys::FLUID);
        let mut block = |key: TagKey<Block, &'static str>| {
            blocks
                .as_ref()
                .and_then(|tags| report.require_tag(tags, &key))
        };
        let overrides_mushroom_light_requirement =
            block(block_tags::OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT);
        let cannot_support_seagrass = block(block_tags::CANNOT_SUPPORT_SEAGRASS);
        let supports_small_dripleaf = block(block_tags::SUPPORTS_SMALL_DRIPLEAF);
        let supports_vegetation = block(block_tags::SUPPORTS_VEGETATION);
        let unstable_bottom_center = block(block_tags::UNSTABLE_BOTTOM_CENTER);
        let supports_lily_pad = block(block_tags::SUPPORTS_LILY_PAD);
        let supports_sugar_cane = block(block_tags::SUPPORTS_SUGAR_CANE);
        let supports_sugar_cane_adjacently = block(block_tags::SUPPORTS_SUGAR_CANE_ADJACENTLY);
        let supports_cactus = block(block_tags::SUPPORTS_CACTUS);
        let supports_lily_pad_fluids = fluids
            .as_ref()
            .and_then(|tags| report.require_tag(tags, &fluid_tags::SUPPORTS_LILY_PAD));
        Some(Resolved::new(Self {
            overrides_mushroom_light_requirement: overrides_mushroom_light_requirement?,
            cannot_support_seagrass: cannot_support_seagrass?,
            supports_small_dripleaf: supports_small_dripleaf?,
            supports_vegetation: supports_vegetation?,
            unstable_bottom_center: unstable_bottom_center?,
            supports_lily_pad: supports_lily_pad?,
            supports_lily_pad_fluids: supports_lily_pad_fluids?,
            supports_sugar_cane: supports_sugar_cane?,
            supports_sugar_cane_adjacently: supports_sugar_cane_adjacently?,
            supports_cactus: supports_cactus?,
        }))
    }
}
