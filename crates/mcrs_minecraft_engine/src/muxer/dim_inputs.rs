use std::mem;

use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Res, ResMut};
use mcrs_minecraft_engine_world::world::channels::TO_DIM_CAPACITY;
use rustc_hash::FxHashMap;

use crate::bus::InboundPlayerPacket;
use crate::channel_types::ToDim;
use crate::column_request::ColumnRequest;
use crate::dim_tick::{TickDimensions, TickDimensionsSet};
use crate::to_coordinator::RoutedInput;
use crate::to_muxer::{DimClosed, DimOpened};

#[derive(Resource, Default)]
pub struct DimInputs(FxHashMap<Entity, RoutedInput>);

impl DimInputs {
    pub fn insert(&mut self, dim: Entity) {
        self.0.entry(dim).or_insert_with(|| RoutedInput::new(dim));
    }

    pub fn remove(&mut self, dim: Entity) {
        self.0.remove(&dim);
    }

    #[must_use]
    pub fn contains(&self, dim: Entity) -> bool {
        self.0.contains_key(&dim)
    }

    pub fn send_control(&mut self, dim: Entity, message: ToDim) -> bool {
        let Some(pending) = self.0.get_mut(&dim) else {
            return false;
        };
        pending.control.push(message);
        true
    }

    pub fn try_send_serverbound(
        &mut self,
        dim: Entity,
        packet: InboundPlayerPacket,
    ) -> Result<(), InboundPlayerPacket> {
        match self.0.get_mut(&dim) {
            Some(pending) if pending.serverbound.len() < TO_DIM_CAPACITY => {
                pending.serverbound.push(packet);
                Ok(())
            }
            _ => Err(packet),
        }
    }

    pub fn send_column_requests(
        &mut self,
        dim: Entity,
        requests: impl IntoIterator<Item = ColumnRequest>,
    ) -> bool {
        let Some(pending) = self.0.get_mut(&dim) else {
            return false;
        };
        pending.column_requests.extend(requests);
        true
    }

    pub fn control(&self, dim: Entity) -> &[ToDim] {
        self.0
            .get(&dim)
            .map_or(&[], |pending| pending.control.as_slice())
    }

    pub fn serverbound(&self, dim: Entity) -> &[InboundPlayerPacket] {
        self.0
            .get(&dim)
            .map_or(&[], |pending| pending.serverbound.as_slice())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub(crate) fn dim_inputs_pending(inputs: Res<DimInputs>) -> bool {
    inputs.0.values().any(|pending| !pending.is_empty())
}

pub fn route_dim_inputs(mut inputs: ResMut<DimInputs>, mut routed: MessageWriter<RoutedInput>) {
    for (&dim, pending) in &mut inputs.0 {
        if !pending.is_empty() {
            routed.write(mem::replace(pending, RoutedInput::new(dim)));
        }
    }
}

pub struct DimInputsPlugin;

impl Plugin for DimInputsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimInputs>();
        app.add_message::<DimOpened>();
        app.add_message::<DimClosed>();
        app.add_message::<RoutedInput>();
        app.add_systems(
            TickDimensions,
            route_dim_inputs
                .run_if(dim_inputs_pending)
                .in_set(TickDimensionsSet::Route),
        );
    }
}
