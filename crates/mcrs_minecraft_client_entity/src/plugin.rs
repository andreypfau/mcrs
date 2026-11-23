use bevy_app::{App, FixedUpdate, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemCondition;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_world::columns::{ColumnChange, ColumnStore};
use mcrs_minecraft_client_world::registries::SessionSet;
use mcrs_minecraft_client_world::session::session_boundary;
use mcrs_minecraft_engine_entity_motion::{
    MotionPass, MotionPassPlugin, MotionRelease, MotionSteps,
};
use mcrs_minecraft_entity_visual::{
    ClientEntityTick, EntityLightSystems, EntityTickPlugin, ServerTicking,
};
use mcrs_minecraft_network::client::ConnectionEnded;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::client_tables::{ClientTables, build_client_tables, clear_client_tables};
use crate::entity_damaged::EntityDamaged;
use crate::entity_light::write_entity_light;
use crate::fuse::burn_fuses;
use crate::motion_tick::{MotionTick, advance_motion_tick};
use crate::network_entities::{NetworkEntities, clear_network_entities};
use crate::orb_interpolation::step_orb_interpolations;
use crate::project_simulated_positions::project_simulated_positions;
use crate::receive;
use crate::remove_spent_entities::remove_spent_entities;
use crate::simulated_step::{
    hold_entities_that_do_not_step, step_simulated_entities, stir_changed_blocks,
};
use crate::sleeping_beds::{BedFacings, build_bed_facings, find_sleeping_beds};
use crate::tracked_velocity::tick_velocities;

pub struct ClientEntityPlugin;

impl Plugin for ClientEntityPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MotionPassPlugin>() {
            app.add_plugins(MotionPassPlugin);
        }
        app.add_message::<ConnectionEnded>()
            .init_resource::<NetworkEntities>()
            .init_resource::<MotionTick>()
            .add_message::<ColumnChange>()
            .add_message::<EntityDamaged>()
            .add_plugins(EntityTickPlugin::<ColumnStore>::default())
            .configure_sets(FixedUpdate, MotionPass.after(ClientEntityTick))
            .add_systems(
                FixedUpdate,
                (
                    tick_velocities.after(ClientEntityTick),
                    build_bed_facings.run_if(
                        resource_exists::<Blocks>
                            .and_then(resource_exists_and_changed::<CurrentRegistries>),
                    ),
                    find_sleeping_beds
                        .after(ClientEntityTick)
                        .run_if(resource_exists::<ColumnStore>)
                        .run_if(resource_exists::<BedFacings>),
                    build_client_tables
                        .run_if(
                            resource_exists::<Blocks>
                                .and_then(resource_exists_and_changed::<CurrentRegistries>),
                        )
                        .before(MotionPass),
                    (advance_motion_tick, stir_changed_blocks).before(MotionPass),
                    step_orb_interpolations
                        .after(ClientEntityTick)
                        .before(MotionPass),
                    hold_entities_that_do_not_step.in_set(MotionRelease),
                    step_simulated_entities
                        .run_if(resource_exists::<ColumnStore>)
                        .run_if(resource_exists::<ClientTables>)
                        .in_set(MotionSteps),
                    (
                        burn_fuses,
                        remove_spent_entities
                            .run_if(resource_exists::<ColumnStore>)
                            .run_if(resource_exists::<ClientTables>),
                    )
                        .chain()
                        .after(MotionPass),
                    project_simulated_positions.before(ClientEntityTick),
                    project_simulated_positions.after(remove_spent_entities),
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    project_simulated_positions.before(EntityLightSystems),
                    write_entity_light
                        .in_set(EntityLightSystems)
                        .run_if(resource_exists::<ColumnStore>),
                ),
            )
            .add_systems(
                Update,
                (
                    clear_network_entities,
                    clear_client_tables,
                    reset_server_ticking,
                )
                    .in_set(SessionSet::Clear)
                    .run_if(session_boundary),
            );
        receive::add_observers(app);
    }
}

fn reset_server_ticking(mut ticking: ResMut<ServerTicking>) {
    ticking.set_if_neq(ServerTicking::default());
}
