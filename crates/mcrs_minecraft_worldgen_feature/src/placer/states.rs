use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelId;

use super::StateMask;

/// The state sets every generator asks about and no feature configures: the
/// same for every feature of a dimension, resolved once at freeze.
#[derive(Clone, Debug, Default)]
pub struct WorldStates {
    pub air: VoxelId,
    pub cave_air: VoxelId,
    pub water: VoxelId,
    pub lava: VoxelId,
    /// `BlockState.isAir()`: air, cave air and void air.
    pub air_states: StateMask,
    /// Every state of the water block, source and flowing.
    pub water_states: StateMask,
    pub lava_states: StateMask,
    /// Every state whose fluid is water, waterlogged blocks included: `isWaterAt`.
    pub water_fluid: StateMask,
    /// Every state holding source water: `FluidState.isSource` over water.
    pub water_source: StateMask,
    pub lava_fluid: StateMask,
    pub any_source_fluid: StateMask,
    pub any_fluid: StateMask,
    pub replaceable: StateMask,
    /// `BlockBehaviour.isSolid`, the legacy flag.
    pub solid: StateMask,
    pub solid_render: StateMask,
    /// `isCollisionShapeFullBlock`, which is what `isFaceSturdy(UP)` reads on a
    /// full cube: the floor most features ask for.
    pub sturdy_up: StateMask,
    /// States whose collision shape is empty.
    pub empty_collision: StateMask,
    pub bedrock: StateMask,
    /// States whose block declares side properties yet never rotates them:
    /// fire and chorus plant.
    pub unrotated: StateMask,
    /// States whose block declares facing or side properties yet never
    /// mirrors them: the anvils on top of `unrotated`.
    pub unmirrored: StateMask,
    /// `BlockState.hasBlockEntity`: the only states a block entity can sit on.
    pub has_block_entity: StateMask,
    /// The block each state id belongs to; a state past the table is its own
    /// block.
    pub block_of_state: Arc<[u32]>,
    /// Per block index, its properties as the digits of its state ids.
    pub layouts: Arc<[BlockLayout]>,
}

/// One property of a block as a digit of its state ids: the values in declared
/// order, each `stride` ids apart.
#[derive(Clone, Debug)]
pub struct PropertyLayout {
    pub name: Arc<str>,
    pub values: Arc<[Arc<str>]>,
    pub stride: u16,
}

#[derive(Clone, Debug, Default)]
pub struct BlockLayout {
    pub base: u16,
    pub properties: Vec<PropertyLayout>,
}

impl BlockLayout {
    pub fn property(&self, name: &str) -> Option<&PropertyLayout> {
        self.properties.iter().find(|p| &*p.name == name)
    }

    pub fn value_index(&self, state: VoxelId, property: &PropertyLayout) -> u16 {
        (state.0 - self.base) / property.stride % property.values.len() as u16
    }

    pub fn with_index(&self, state: VoxelId, property: &PropertyLayout, index: u16) -> VoxelId {
        let shift =
            (index as i32 - self.value_index(state, property) as i32) * property.stride as i32;
        VoxelId((state.0 as i32 + shift) as u16)
    }

    /// `BlockState.trySetValue`: a property this block lacks, or a value
    /// outside it, leaves the state alone.
    pub fn try_set(&self, state: VoxelId, name: &str, value: &str) -> VoxelId {
        let Some(property) = self.property(name) else {
            return state;
        };
        match property.values.iter().position(|held| &**held == value) {
            Some(index) => self.with_index(state, property, index as u16),
            None => state,
        }
    }

    /// `BlockState.withPropertiesOf`: every property `source` shares by name
    /// and value text, copied over.
    pub fn with_properties_of(
        &self,
        state: VoxelId,
        from: &BlockLayout,
        source: VoxelId,
    ) -> VoxelId {
        from.properties.iter().fold(state, |state, property| {
            let value = &property.values[from.value_index(source, property) as usize];
            self.try_set(state, &property.name, value)
        })
    }
}

impl WorldStates {
    pub fn layout_of(&self, state: VoxelId) -> Option<&BlockLayout> {
        self.layouts.get(self.block_of(state) as usize)
    }

    /// `SpeleothemUtils.isEmptyOrWater`: air, or the water block itself.
    pub fn is_empty_or_water(&self, state: VoxelId) -> bool {
        self.air_states.contains(state.0 as usize) || self.water_states.contains(state.0 as usize)
    }

    pub fn is_empty_or_water_or_lava(&self, state: VoxelId) -> bool {
        self.is_empty_or_water(state) || self.lava_states.contains(state.0 as usize)
    }

    pub fn block_of(&self, state: VoxelId) -> u32 {
        self.block_of_state
            .get(state.0 as usize)
            .copied()
            .unwrap_or(state.0 as u32)
    }
}
