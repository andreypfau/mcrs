use bevy_ecs::prelude::Resource;
use mcrs_voxel_math::voxel_shape::VoxelShape;
use mcrs_voxel_storage::VoxelId;

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
