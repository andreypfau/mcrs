use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_core::tag::TagPhase;
use mcrs_minecraft_light::block::{LightProperties, LightRegistry, SpecialBlocks};
use mcrs_minecraft_light::level::LightLevel;
use mcrs_minecraft_protocol::BlockStateId;
use mcrs_minecraft_world::block::definition::{BlockStateFlags, Blocks, ShapeId};
use mcrs_minecraft_world::transition_to_playing;
use mcrs_voxel_math::voxel_shape::{ShapeRegistry, VoxelShape};
use mcrs_voxel_storage::VoxelId;
use rustc_hash::FxHashMap;
use std::sync::{Arc, OnceLock};

#[derive(Resource, Clone)]
pub struct BlockLightRegistry(pub Arc<LightRegistry>);

pub struct BlockLightTablePlugin;

impl Plugin for BlockLightTablePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            insert_block_light_registry
                .after(TagPhase::Freeze)
                .before(transition_to_playing)
                .run_if(|| !crate::lighting_disabled()),
        );
    }
}

fn insert_block_light_registry(mut commands: Commands, blocks: Res<Blocks>) {
    commands.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}

/// Interning a shape leaks it, so the table is built once and every app in the
/// process shares it. There is one asset corpus per process, so a second app
/// would derive the same rows anyway.
pub fn block_light_registry(blocks: &Blocks) -> Arc<LightRegistry> {
    static REGISTRY: OnceLock<Arc<LightRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(build(blocks))).clone()
}

fn build(blocks: &Blocks) -> LightRegistry {
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
    use bevy_app::TaskPoolPlugin;
    use bevy_asset::{AssetPlugin, AssetServer};
    use mcrs_minecraft_world::block::definition::load_block_definitions;

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
            let (definitions, _) =
                load_block_definitions(&asset_server).expect("the block definition corpus loads");
            Blocks(Arc::new(definitions))
        })
    }

    fn default_state(block: &str) -> VoxelId {
        corpus()
            .block(block)
            .expect("the block is declared")
            .default_state_id
            .into()
    }

    #[test]
    fn shaped_states_are_exactly_the_states_the_corpus_flags() {
        let blocks = corpus();
        let registry = block_light_registry(blocks);
        let mut flagged = 0usize;
        for index in 0..blocks.state_count() {
            let id = VoxelId(index as u16);
            let expected = blocks
                .state(id.into())
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

    #[test]
    fn a_slab_is_shaped_and_a_full_block_is_not() {
        let blocks = corpus();
        let registry = block_light_registry(blocks);
        let slab = blocks.block("minecraft:oak_slab").expect("oak slab");
        let bottom: VoxelId = slab
            .with_text(slab.default_state_id, "type", "bottom")
            .expect("a bottom slab")
            .into();
        assert!(registry.get(bottom).occlusion.is_some());

        let stone = default_state("minecraft:stone");
        assert!(registry.get(stone).occlusion.is_none());
        assert_eq!(registry.get(stone).dampening, 15);
        assert_eq!(registry.get(default_state("minecraft:air")).dampening, 0);
    }

    #[test]
    fn emission_comes_from_the_corpus() {
        let blocks = corpus();
        let registry = block_light_registry(blocks);
        let torch = default_state("minecraft:torch");
        let expected = blocks.state(torch.into()).light_emission;
        assert!(expected > 0, "a torch emits light");
        assert_eq!(registry.get(torch).emission.get(), expected);
        assert_eq!(
            registry.get(default_state("minecraft:stone")).emission,
            LightLevel::ZERO
        );
    }

    #[test]
    fn the_filler_ids_sit_past_the_corpus() {
        let blocks = corpus();
        let registry = block_light_registry(blocks);
        assert_eq!(registry.unloaded(), VoxelId(blocks.state_count() as u16));
        assert_eq!(registry.outside(), VoxelId(blocks.state_count() as u16 + 1));
        assert_eq!(registry.get(registry.unloaded()).dampening, 15);
        assert_eq!(registry.get(registry.outside()).dampening, 0);
    }
}
