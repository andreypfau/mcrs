use bevy_ecs::prelude::{Commands, Res, Resource};
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_protocol::BlockStateId;
use mcrs_vanilla::block::behaviour::Properties;
use mcrs_vanilla::block::Block;

#[derive(Resource, Debug)]
pub struct BlockLightTable {
    pub emission: Box<[u8]>,
    pub dampening: Box<[u8]>,
    pub occlusion: Box<[&'static VoxelShape]>,
    pub flags: Box<[u8]>,
}

impl BlockLightTable {
    #[inline]
    pub fn len(&self) -> usize {
        self.emission.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.emission.is_empty()
    }

    #[inline]
    pub fn emission_for(&self, state: BlockStateId) -> u8 {
        self.emission.get(state.0 as usize).copied().unwrap_or(0)
    }

    #[inline]
    pub fn dampening_for(&self, state: BlockStateId) -> u8 {
        self.dampening.get(state.0 as usize).copied().unwrap_or(0)
    }

    #[inline]
    pub fn occlusion_for(&self, state: BlockStateId) -> &'static VoxelShape {
        self.occlusion
            .get(state.0 as usize)
            .copied()
            .unwrap_or_else(VoxelShape::empty)
    }

    #[inline]
    pub fn flags_for(&self, state: BlockStateId) -> u8 {
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

fn compute_flags(props: &Properties, dampening: u8, occlusion: &'static VoxelShape) -> u8 {
    let mut f = 0u8;
    if !occlusion.is_empty() && !occlusion.occludes_full_block() {
        f |= flag_bits::IS_CONDITIONALLY_OPAQUE;
    }
    if dampening == 0 {
        f |= flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    }
    if props.can_occlude && dampening == 15 {
        f |= flag_bits::IS_SOLID_OPAQUE;
    }
    if props.has_collision {
        f |= flag_bits::IS_MOTION_BLOCKING;
    }
    if !props.is_air {
        f |= flag_bits::IS_NOT_AIR;
    }
    f
}

pub fn build_block_light_table(
    mut commands: Commands,
    blocks: Res<StaticRegistry<Block>>,
) {
    debug_assert!(
        blocks.frozen(),
        "BlockLightTable::build called before registry freeze"
    );

    let mut total_states = 0usize;
    for (_id, _loc, block) in blocks.iter() {
        let base = block.base_state_id().0 as usize;
        let span = base + block.state_count as usize;
        if span > total_states {
            total_states = span;
        }
    }

    let mut emission = vec![0u8; total_states].into_boxed_slice();
    let mut dampening = vec![0u8; total_states].into_boxed_slice();
    let mut occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); total_states].into_boxed_slice();
    let mut flags = vec![0u8; total_states].into_boxed_slice();

    for (_id, _loc, block) in blocks.iter() {
        let base = block.base_state_id().0 as usize;
        for offset in 0..block.state_count {
            let state_id = block
                .base_state_id()
                .0
                .checked_add(offset)
                .expect("block state ID overflow during BlockLightTable build");
            let state = BlockStateId(state_id);
            let idx = base + offset as usize;
            emission[idx] = block.properties.light_emission.eval(block, state);
            dampening[idx] = block.properties.light_dampening.eval(block, state);
            occlusion[idx] = block.properties.occlusion.eval(block, state);
            flags[idx] = compute_flags(&block.properties, dampening[idx], occlusion[idx]);
        }
    }

    tracing::info!(state_count = total_states, "built BlockLightTable");
    commands.insert_resource(BlockLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_core::resource_location::ResourceLocation;
    use mcrs_vanilla::block::behaviour::{LightSpec, OcclusionSpec};

    fn make_block(
        id: &'static str,
        protocol_id: u16,
        base_state_id: u16,
        state_count: u16,
        properties: &'static Properties,
    ) -> &'static Block {
        Box::leak(Box::new(Block {
            identifier: ResourceLocation::new_static(id),
            protocol_id,
            properties,
            default_state_id: BlockStateId(base_state_id),
            layout: None,
            state_count,
        }))
    }

    fn build_table(blocks: Vec<&'static Block>) -> BlockLightTable {
        let mut registry: StaticRegistry<Block> = StaticRegistry::new();
        for block in blocks {
            registry.register(block.identifier, block);
        }
        registry.freeze();

        let mut total_states = 0usize;
        for (_id, _loc, block) in registry.iter() {
            let base = block.base_state_id().0 as usize;
            let span = base + block.state_count as usize;
            if span > total_states {
                total_states = span;
            }
        }

        let mut emission = vec![0u8; total_states].into_boxed_slice();
        let mut dampening = vec![0u8; total_states].into_boxed_slice();
        let mut occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); total_states].into_boxed_slice();
        let mut flags = vec![0u8; total_states].into_boxed_slice();

        for (_id, _loc, block) in registry.iter() {
            let base = block.base_state_id().0 as usize;
            for offset in 0..block.state_count {
                let state_id = block
                    .base_state_id()
                    .0
                    .checked_add(offset)
                    .expect("block state ID overflow during BlockLightTable build");
                let state = BlockStateId(state_id);
                let idx = base + offset as usize;
                emission[idx] = block.properties.light_emission.eval(block, state);
                dampening[idx] = block.properties.light_dampening.eval(block, state);
                occlusion[idx] = block.properties.occlusion.eval(block, state);
                flags[idx] = compute_flags(&block.properties, dampening[idx], occlusion[idx]);
            }
        }

        BlockLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        }
    }

    #[test]
    fn build_constant_emitter_block() {
        static PROPS: Properties =
            Properties::new().with_light_emission(LightSpec::Const(15));
        let block = make_block("test:emitter", 1, 100, 4, &PROPS);
        let table = build_table(vec![block]);
        for i in 0..4u16 {
            let state = BlockStateId(100 + i);
            assert_eq!(table.emission_for(state), 15);
        }
    }

    #[test]
    fn build_per_state_emitter_block() {
        // The fn pointer receives the OWNING `&Block` reference (Pitfall #9).
        // We compute emission as (state.0 - base) so the test catches the wrong-block
        // capture regression: a wrong reference would give wrong base subtraction.
        fn per_state(_b: &Block, s: BlockStateId) -> u8 {
            (s.0 - 200) as u8 & 0x0F
        }
        static PROPS: Properties = Properties::new()
            .with_light_emission(LightSpec::PerState(per_state))
            .with_light_dampening(LightSpec::Const(0));
        let block = make_block("test:per_state", 2, 200, 16, &PROPS);
        let table = build_table(vec![block]);
        for i in 0..16u16 {
            let state = BlockStateId(200 + i);
            assert_eq!(
                table.emission_for(state),
                i as u8,
                "state {state:?} expected emission {i}"
            );
        }
    }

    #[test]
    fn build_air_defaults() {
        static PROPS: Properties = Properties::new().air();
        let block = make_block("test:air", 0, 0, 1, &PROPS);
        let table = build_table(vec![block]);
        let s = BlockStateId(0);
        assert_eq!(table.emission_for(s), 0);
        assert_eq!(table.dampening_for(s), 0);
        let f = table.flags_for(s);
        assert_eq!(f & flag_bits::IS_NOT_AIR, 0);
        assert_ne!(f & flag_bits::PROPAGATES_SKYLIGHT_DOWN, 0);
    }

    #[test]
    fn build_solid_defaults() {
        static PROPS: Properties = Properties::new();
        let block = make_block("test:solid", 3, 50, 1, &PROPS);
        let table = build_table(vec![block]);
        let s = BlockStateId(50);
        assert_eq!(table.dampening_for(s), 15);
        let f = table.flags_for(s);
        assert_ne!(f & flag_bits::IS_SOLID_OPAQUE, 0, "expected IS_SOLID_OPAQUE bit");
        assert_ne!(f & flag_bits::IS_NOT_AIR, 0);
        assert_ne!(f & flag_bits::IS_MOTION_BLOCKING, 0);
    }

    #[test]
    fn build_partial_defaults_no_collision() {
        static PROPS: Properties = Properties::new().no_collision();
        let block = make_block("test:partial", 4, 75, 1, &PROPS);
        let table = build_table(vec![block]);
        let s = BlockStateId(75);
        assert_eq!(table.dampening_for(s), 1);
        let f = table.flags_for(s);
        assert_eq!(f & flag_bits::IS_SOLID_OPAQUE, 0);
        assert_eq!(f & flag_bits::IS_MOTION_BLOCKING, 0);
    }

    #[test]
    fn flags_byte_layout() {
        assert_eq!(flag_bits::IS_CONDITIONALLY_OPAQUE, 1);
        assert_eq!(flag_bits::PROPAGATES_SKYLIGHT_DOWN, 2);
        assert_eq!(flag_bits::IS_SOLID_OPAQUE, 4);
        assert_eq!(flag_bits::IS_MOTION_BLOCKING, 8);
        assert_eq!(flag_bits::IS_NOT_AIR, 16);
    }

    #[test]
    fn table_len_reports_total_states() {
        static AIR_PROPS: Properties = Properties::new().air();
        static SOLID_PROPS: Properties = Properties::new();
        let air = make_block("test:t_air", 0, 0, 1, &AIR_PROPS);
        let solid = make_block("test:t_solid", 1, 1, 16, &SOLID_PROPS);
        let table = build_table(vec![air, solid]);
        assert_eq!(table.len(), 17);
        assert!(!table.is_empty());
    }
}
