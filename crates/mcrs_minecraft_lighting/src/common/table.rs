use bevy_ecs::prelude::{Commands, Res, Resource};
use bevy_math::Vec3;
use mcrs_vanilla::block::definition::{BlockStateData, BlockStateFlags, Blocks, ShapeId};
use mcrs_voxel_math::voxel_shape::{
    Aabb, ShapeRegistry, ShapeRepr, VoxelShape, discrete::DiscreteShape,
};
use mcrs_voxel_storage::VoxelId;
use rustc_hash::FxHashMap;

#[derive(Resource, Debug, Default, Clone)]
pub struct BlockStateLightTable {
    pub emission: Box<[u8]>,
    pub dampening: Box<[u8]>,
    pub occlusion: Box<[&'static VoxelShape]>,
    pub flags: Box<[u8]>,
}

impl BlockStateLightTable {
    #[inline]
    pub fn len(&self) -> usize {
        self.emission.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.emission.is_empty()
    }

    #[inline]
    pub fn emission_for(&self, state: VoxelId) -> u8 {
        self.emission.get(state.0 as usize).copied().unwrap_or(0)
    }

    #[inline]
    pub fn dampening_for(&self, state: VoxelId) -> u8 {
        self.dampening.get(state.0 as usize).copied().unwrap_or(0)
    }

    #[inline]
    pub fn occlusion_for(&self, state: VoxelId) -> &'static VoxelShape {
        self.occlusion
            .get(state.0 as usize)
            .copied()
            .unwrap_or_else(VoxelShape::empty)
    }

    #[inline]
    pub fn flags_for(&self, state: VoxelId) -> u8 {
        self.flags.get(state.0 as usize).copied().unwrap_or(0)
    }
}

pub mod flag_bits {
    pub const IS_CONDITIONALLY_OPAQUE: u8 = 1 << 0;
    pub const PROPAGATES_SKYLIGHT_DOWN: u8 = 1 << 1;
    pub const IS_SOLID_OPAQUE: u8 = 1 << 2;
    pub const IS_MOTION_BLOCKING: u8 = 1 << 3;
    pub const IS_NOT_AIR: u8 = 1 << 4;
}

fn compute_flags(state: &BlockStateData, occlusion: &VoxelShape) -> u8 {
    let mut f = 0u8;
    if !occlusion.is_empty() && !occlusion.occludes_full_block() {
        f |= flag_bits::IS_CONDITIONALLY_OPAQUE;
    }
    if state
        .flags
        .contains(BlockStateFlags::PROPAGATES_SKYLIGHT_DOWN)
    {
        f |= flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    }
    if state.flags.contains(BlockStateFlags::IS_SOLID_RENDER) {
        f |= flag_bits::IS_SOLID_OPAQUE;
    }
    if !state.flags.contains(BlockStateFlags::IS_AIR) {
        f |= flag_bits::IS_NOT_AIR;
    }
    f
}

const UNIT_CUBE: Aabb = Aabb {
    min: Vec3::ZERO,
    max: Vec3::ONE,
};

/// The light engine reads face occlusion, and a partial shape has no face
/// projection yet, so anything short of the full cube interns as a shape whose
/// faces occlude nothing. That is the conservative direction: light passes
/// where vanilla might cull it, never the other way round.
fn intern_occlusion(boxes: &[Aabb], shapes: &mut ShapeRegistry) -> &'static VoxelShape {
    if boxes.is_empty() {
        return VoxelShape::empty();
    }
    if boxes.len() == 1 && boxes[0] == UNIT_CUBE {
        return VoxelShape::block();
    }
    let bounds = boxes.iter().skip(1).fold(boxes[0], |acc, b| Aabb {
        min: acc.min.min(b.min),
        max: acc.max.max(b.max),
    });
    shapes.intern(VoxelShape {
        repr: ShapeRepr::Discrete(DiscreteShape::empty_with_bounds(bounds)),
        bounds,
        occludes_full_block: false,
        face_cache: [VoxelShape::empty(); 6],
    })
}

/// One row per block state, straight off the corpus. Every state the game has
/// is in the table: a state the light engine cannot find is a state it would
/// treat as air.
pub fn build_block_light_table(mut commands: Commands, blocks: Res<Blocks>) {
    let total_states = blocks.state_count();
    let mut emission = vec![0u8; total_states].into_boxed_slice();
    let mut dampening = vec![0u8; total_states].into_boxed_slice();
    let mut occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); total_states].into_boxed_slice();
    let mut flags = vec![0u8; total_states].into_boxed_slice();

    let mut shapes = ShapeRegistry::new();
    let mut interned: FxHashMap<ShapeId, &'static VoxelShape> = FxHashMap::default();

    for index in 0..total_states {
        let state = blocks.state(VoxelId(index as u16));
        let shape = match interned.get(&state.occlusion_shape) {
            Some(shape) => *shape,
            None => {
                let shape = intern_occlusion(blocks.shape(state.occlusion_shape), &mut shapes);
                interned.insert(state.occlusion_shape, shape);
                shape
            }
        };
        emission[index] = state.light_emission;
        dampening[index] = state.light_dampening;
        occlusion[index] = shape;
        flags[index] = compute_flags(state, shape);
    }

    tracing::info!(
        state_count = total_states,
        shapes = shapes.len(),
        "built BlockStateLightTable"
    );
    commands.insert_resource(BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Startup, TaskPoolPlugin};
    use bevy_asset::{AssetPlugin, AssetServer};
    use mcrs_vanilla::block::definition::load_block_definitions;
    use std::sync::{Arc, OnceLock};

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

    fn table() -> &'static BlockStateLightTable {
        static TABLE: OnceLock<BlockStateLightTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            let mut app = App::new();
            app.insert_resource(corpus().clone());
            app.add_systems(Startup, build_block_light_table);
            app.update();
            app.world().resource::<BlockStateLightTable>().clone()
        })
    }

    fn state_of(block: &str) -> VoxelId {
        corpus()
            .block(block)
            .expect("the block is declared")
            .default_state_id
    }

    #[test]
    fn every_state_the_game_has_is_in_the_table() {
        assert_eq!(table().len(), corpus().state_count());
    }

    #[test]
    fn a_full_block_dampens_and_occludes() {
        let stone = state_of("minecraft:stone");
        assert_eq!(table().dampening_for(stone), 15);
        assert_eq!(table().emission_for(stone), 0);
        assert!(table().occlusion_for(stone).occludes_full_block());
        let flags = table().flags_for(stone);
        assert!(flags & flag_bits::IS_SOLID_OPAQUE != 0);
        assert!(flags & flag_bits::IS_NOT_AIR != 0);
        assert!(flags & flag_bits::IS_CONDITIONALLY_OPAQUE == 0);
        assert!(flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN == 0);
    }

    #[test]
    fn an_emitter_carries_its_light_and_lets_the_sky_through() {
        let torch = state_of("minecraft:torch");
        assert_eq!(table().emission_for(torch), 14);
        assert_eq!(table().dampening_for(torch), 0);
        assert!(table().occlusion_for(torch).is_empty());
        assert!(table().flags_for(torch) & flag_bits::PROPAGATES_SKYLIGHT_DOWN != 0);
    }

    #[test]
    fn air_is_the_only_thing_flagged_as_air() {
        for block in ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"] {
            let flags = table().flags_for(state_of(block));
            assert!(flags & flag_bits::IS_NOT_AIR == 0, "{block}");
        }
        assert!(table().flags_for(state_of("minecraft:barrier")) & flag_bits::IS_NOT_AIR != 0);
    }

    #[test]
    fn a_partial_occluder_is_conditionally_opaque() {
        let slab = corpus().block("minecraft:oak_slab").unwrap();
        let bottom = slab
            .with_text(slab.default_state_id, "type", "bottom")
            .expect("a bottom slab");
        let flags = table().flags_for(bottom);
        assert!(flags & flag_bits::IS_CONDITIONALLY_OPAQUE != 0);
        assert!(flags & flag_bits::IS_SOLID_OPAQUE == 0);
        assert!(!table().occlusion_for(bottom).is_empty());
    }

    #[test]
    fn a_state_beyond_the_table_reads_as_nothing() {
        let beyond = VoxelId(u16::MAX);
        assert_eq!(table().emission_for(beyond), 0);
        assert_eq!(table().dampening_for(beyond), 0);
        assert_eq!(table().flags_for(beyond), 0);
        assert!(table().occlusion_for(beyond).is_empty());
    }
}
