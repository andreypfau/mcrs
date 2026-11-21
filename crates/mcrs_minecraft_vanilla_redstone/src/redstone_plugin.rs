use core::marker::PhantomData;

use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::resource_exists;
use mcrs_minecraft_engine::PoseSystems;
use mcrs_minecraft_engine::block_simulation::{BlockMemory, BlockRules, BlockShared};
use mcrs_minecraft_engine::use_item_on_request::BlockUseSet;
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_entity_motion::MotionPass;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_redstone::{HasRedstone, HasRedstoneMemory, HasRedstoneSnapshot, RedstoneRules};
use mcrs_minecraft_simulation::BlockBehaviour;

use crate::comparator_store::{persist_comparator_outputs, restore_comparator_outputs};
use crate::container_signal::publish_container_signals;
use crate::daylight_signal::publish_daylight;
use crate::moving_blocks::push_entities_with_moving_blocks;
use crate::moving_store::{
    CarriedRecords, capture_carried_entities, record_moving_blocks, restore_moving_blocks,
};
use crate::plate_presence::{index_loaded_sensors, publish_plate_presence};
use crate::use_blocks::use_redstone_blocks;

pub struct RedstonePlugin<B> {
    pub rules: RedstoneRules,
    marker: PhantomData<fn() -> B>,
}

impl<B> RedstonePlugin<B> {
    #[must_use]
    pub fn with_rules(rules: RedstoneRules) -> Self {
        Self {
            rules,
            marker: PhantomData,
        }
    }
}

impl<B> Default for RedstonePlugin<B> {
    fn default() -> Self {
        Self::with_rules(RedstoneRules::JAVA)
    }
}

impl<B> Clone for RedstonePlugin<B> {
    fn clone(&self) -> Self {
        Self::with_rules(self.rules)
    }
}

impl<B> Plugin for RedstonePlugin<B>
where
    B: BlockBehaviour<WorldKinds> + HasRedstone + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    fn build(&self, app: &mut App) {
        if let Some(mut rules) = app.world_mut().get_resource_mut::<BlockRules<B>>() {
            rules.0.redstone_mut().set_rules(self.rules);
        }
        app.add_systems(
            FixedUpdate,
            use_redstone_blocks::<B>
                .in_set(BlockUseSet)
                .run_if(resource_exists::<BlockRules<B>>),
        );
        app.add_systems(
            FixedUpdate,
            (
                index_loaded_sensors::<B>,
                restore_comparator_outputs::<B>,
                restore_moving_blocks::<B>,
                publish_plate_presence::<B>,
                publish_daylight::<B>,
                publish_container_signals::<B>,
            )
                .chain()
                .in_set(TickSet::Simulate)
                .after(MotionPass)
                .run_if(resource_exists::<BlockRules<B>>)
                .run_if(resource_exists::<BlockMemory<B>>)
                .run_if(resource_exists::<BlockShared<B>>),
        );
        app.add_systems(
            FixedUpdate,
            push_entities_with_moving_blocks::<B>
                .in_set(TickSet::Simulate)
                .before(MotionPass)
                .before(PoseSystems::Select)
                .run_if(resource_exists::<BlockMemory<B>>),
        );
        app.init_resource::<CarriedRecords>();
        app.add_systems(
            FixedPostUpdate,
            capture_carried_entities::<B>
                .before(VoxelUpdateSet::NetworkSync)
                .run_if(resource_exists::<BlockRules<B>>)
                .run_if(resource_exists::<BlockMemory<B>>),
        );
        app.add_systems(
            FixedPostUpdate,
            (record_moving_blocks::<B>, persist_comparator_outputs::<B>)
                .chain()
                .after(VoxelUpdateSet::NetworkSync)
                .run_if(resource_exists::<BlockRules<B>>)
                .run_if(resource_exists::<BlockMemory<B>>),
        );
    }
}
