use std::sync::Arc;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_resource::Id;

use super::StateMask;

#[derive(Clone, Copy, Debug)]
pub struct StateCount(pub usize);

impl Default for StateCount {
    fn default() -> Self {
        Self(usize::from(u16::MAX) + 1)
    }
}

#[derive(Clone, Debug, Default)]
pub struct WorldStates {
    pub state_count: StateCount,
    pub air: VoxelId,
    pub cave_air: VoxelId,
    pub water: VoxelId,
    pub lava: VoxelId,
    pub air_states: StateMask,
    pub water_states: StateMask,
    pub lava_states: StateMask,
    pub water_fluid: StateMask,
    pub water_source: StateMask,
    pub freezable_water: StateMask,
    pub lava_fluid: StateMask,
    pub any_source_fluid: StateMask,
    pub any_fluid: StateMask,
    pub replaceable: StateMask,
    pub solid: StateMask,
    pub solid_render: StateMask,
    pub sturdy_up: StateMask,
    pub center_down: StateMask,
    pub empty_collision: StateMask,
    pub bedrock: StateMask,
    pub unrotated: StateMask,
    pub unmirrored: StateMask,
    pub has_block_entity: StateMask,
    pub block_of_state: Arc<[Id<Block>]>,
    pub layouts: Arc<[BlockLayout]>,
    pub fluid_states: Arc<[u16]>,
}

#[derive(Clone, Debug)]
pub struct PropertyLayout {
    pub name: Arc<str>,
    pub values: Arc<[Arc<str>]>,
    pub stride: u16,
}

pub fn digit(state: VoxelId, base: u16, stride: u16, count: u16) -> u16 {
    (state.0 - base) / stride % count
}

pub fn with_digit(state: VoxelId, base: u16, stride: u16, count: u16, index: u16) -> VoxelId {
    let shift = (index as i32 - digit(state, base, stride, count) as i32) * stride as i32;
    VoxelId((state.0 as i32 + shift) as u16)
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
        digit(
            state,
            self.base,
            property.stride,
            property.values.len() as u16,
        )
    }

    pub fn with_index(&self, state: VoxelId, property: &PropertyLayout, index: u16) -> VoxelId {
        with_digit(
            state,
            self.base,
            property.stride,
            property.values.len() as u16,
            index,
        )
    }

    pub fn try_set(&self, state: VoxelId, name: &str, value: &str) -> VoxelId {
        let Some(property) = self.property(name) else {
            return state;
        };
        match property.values.iter().position(|held| &**held == value) {
            Some(index) => self.with_index(state, property, index as u16),
            None => state,
        }
    }

    pub fn with_properties_of(
        &self,
        state: VoxelId,
        from: &BlockLayout,
        source: VoxelId,
    ) -> VoxelId {
        from.properties.iter().fold(state, |state, property| {
            match property
                .values
                .get(usize::from(from.value_index(source, property)))
            {
                Some(value) => self.try_set(state, &property.name, value),
                None => state,
            }
        })
    }
}

impl WorldStates {
    pub fn fluid_state_of(&self, state: VoxelId) -> u16 {
        self.fluid_states
            .get(usize::from(state.0))
            .copied()
            .unwrap_or(0)
    }

    pub fn layout_of(&self, state: VoxelId) -> Option<&BlockLayout> {
        self.layouts.get(self.block_of(state)?.index())
    }

    pub fn with_properties_of(&self, target: VoxelId, source: VoxelId) -> VoxelId {
        match (self.layout_of(target), self.layout_of(source)) {
            (Some(into), Some(from)) => into.with_properties_of(target, from, source),
            _ => target,
        }
    }

    pub fn is_empty_or_water(&self, state: VoxelId) -> bool {
        self.air_states.contains(state.0 as usize) || self.water_states.contains(state.0 as usize)
    }

    pub fn is_empty_or_water_or_lava(&self, state: VoxelId) -> bool {
        self.is_empty_or_water(state) || self.lava_states.contains(state.0 as usize)
    }

    pub fn block_of(&self, state: VoxelId) -> Option<Id<Block>> {
        self.block_of_state.get(usize::from(state.0)).copied()
    }
}
