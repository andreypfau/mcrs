use std::sync::{Arc, OnceLock};

use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags, ShapeId};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::voxel_shape::{ShapeRegistry, VoxelShape};
use mcrs_minecraft_registry::BlockStateId;
use rustc_hash::FxHashMap;

use crate::block::{LightProperties, LightRegistry, SpecialBlocks};
use crate::level::LightLevel;

#[cfg(feature = "bevy")]
#[derive(bevy_ecs::resource::Resource, Clone)]
pub struct BlockLightRegistry(pub Arc<LightRegistry>);

/// Interning a shape leaks it, so the table is built once and every app in the
/// process shares it. There is one asset corpus per process, so a second app
/// would derive the same rows anyway.
pub fn block_light_registry(blocks: &BlockDefinitions) -> Arc<LightRegistry> {
    static REGISTRY: OnceLock<Arc<LightRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(build(blocks))).clone()
}

fn build(blocks: &BlockDefinitions) -> LightRegistry {
    let state_count = blocks.state_count();
    assert!(
        state_count + 2 <= u16::MAX as usize,
        "the corpus leaves no room for the two filler ids"
    );

    let mut shapes = ShapeRegistry::new();
    let mut interned: FxHashMap<ShapeId, &'static VoxelShape> = FxHashMap::default();
    let mut properties = Vec::with_capacity(state_count + 2);

    for index in 0..state_count {
        let state = blocks.state(BlockStateId(index as u16));
        let occlusion = state
            .flags
            .contains(BlockStateFlags::USE_SHAPE_FOR_LIGHT_OCCLUSION)
            .then(|| {
                *interned.entry(state.occlusion_shape).or_insert_with(|| {
                    shapes.intern(VoxelShape::from_boxes(blocks.shape(state.occlusion_shape)))
                })
            });
        properties.push(LightProperties {
            dampening: state.light_dampening,
            emission: LightLevel::new(state.light_emission),
            occlusion,
        });
    }

    // The two filler ids sit past the last corpus state, so no palette lookup
    // and no protocol id can ever name them.
    properties.push(LightProperties::SOLID);
    properties.push(LightProperties::AIR);

    tracing::info!(
        states = state_count,
        shapes = shapes.len(),
        "built block light registry"
    );

    LightRegistry::new(
        properties,
        SpecialBlocks {
            unloaded: VoxelId(state_count as u16),
            outside: VoxelId(state_count as u16 + 1),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_app::TaskPoolPlugin;
    use bevy_asset::{AssetPlugin, AssetServer};
    use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
    use mcrs_minecraft_registry::static_report::shipped_report;

    fn corpus() -> &'static Blocks {
        static CORPUS: OnceLock<Blocks> = OnceLock::new();
        CORPUS.get_or_init(|| {
            let mut app = App::new();
            app.add_plugins(TaskPoolPlugin::default());
            app.add_plugins(AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            });
            let asset_server = app.world().resource::<AssetServer>().clone();
            let blocks = shipped_report()
                .registry_of(mcrs_minecraft_block::keys::BLOCK)
                .expect("the registries report has blocks");
            let (definitions, _) = load_block_definitions(&asset_server, &blocks)
                .expect("the block definition corpus loads");
            Blocks(Arc::new(definitions))
        })
    }

    fn default_state(block: &str) -> VoxelId {
        VoxelId(
            corpus()
                .block(block)
                .expect("the block is declared")
                .default_state_id
                .0,
        )
    }

    fn shaped_states_are_exactly_the_states_the_corpus_flags(
        blocks: &Blocks,
        registry: &LightRegistry,
    ) {
        let mut flagged = 0usize;
        for index in 0..blocks.state_count() {
            let id = VoxelId(index as u16);
            let expected = blocks
                .state(id.0.into())
                .flags
                .contains(BlockStateFlags::USE_SHAPE_FOR_LIGHT_OCCLUSION);
            assert_eq!(
                registry.get(id).occlusion.is_some(),
                expected,
                "state {index} shaped-ness"
            );
            flagged += expected as usize;
        }
        assert!(flagged > 0, "the corpus flags no state for light occlusion");
        assert!(flagged < blocks.state_count() / 2);
    }

    fn a_slab_is_shaped_and_a_full_block_is_not(blocks: &Blocks, registry: &LightRegistry) {
        let slab = blocks.block("minecraft:oak_slab").expect("oak slab");
        let bottom = VoxelId(
            slab.with_text(slab.default_state_id, "type", "bottom")
                .expect("a bottom slab")
                .0,
        );
        assert!(registry.get(bottom).occlusion.is_some());

        let stone = default_state("minecraft:stone");
        assert!(registry.get(stone).occlusion.is_none());
        assert_eq!(registry.get(stone).dampening, 15);
        assert_eq!(registry.get(default_state("minecraft:air")).dampening, 0);
    }

    fn emission_comes_from_the_corpus(blocks: &Blocks, registry: &LightRegistry) {
        let torch = default_state("minecraft:torch");
        let expected = blocks.state(torch.0.into()).light_emission;
        assert!(expected > 0, "a torch emits light");
        assert_eq!(registry.get(torch).emission.get(), expected);
        assert_eq!(
            registry.get(default_state("minecraft:stone")).emission,
            LightLevel::ZERO
        );
    }

    fn the_filler_ids_sit_past_the_corpus(blocks: &Blocks, registry: &LightRegistry) {
        assert_eq!(registry.unloaded(), VoxelId(blocks.state_count() as u16));
        assert_eq!(registry.outside(), VoxelId(blocks.state_count() as u16 + 1));
        assert_eq!(registry.get(registry.unloaded()).dampening, 15);
        assert_eq!(registry.get(registry.outside()).dampening, 0);
    }

    #[test]
    fn the_light_registry_is_built_from_the_corpus() {
        let blocks = corpus();
        let registry = block_light_registry(blocks);
        shaped_states_are_exactly_the_states_the_corpus_flags(blocks, &registry);
        a_slab_is_shaped_and_a_full_block_is_not(blocks, &registry);
        emission_comes_from_the_corpus(blocks, &registry);
        the_filler_ids_sit_past_the_corpus(blocks, &registry);
    }
}
