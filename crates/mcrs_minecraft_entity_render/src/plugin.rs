use bevy::core_pipeline::schedule::Core3d;
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSystems};
use mcrs_minecraft_client_entity::{ClientEntityTick, ServerTicking};
use mcrs_minecraft_render::WorldPass;

use crate::main_world::{
    EntityClock, EntityVm, advance_clock, assign_slots, frame_clock, release_slot, write_inputs,
    write_layers, write_ticks,
};
use crate::render_world::{EntityRenderState, draw_entities, extract_entities, prepare_entities};
use crate::slot_upload::SlotArena;

/// The systems that keep the slot of each client entity, in `PostUpdate`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntitySlotSystems;

/// Draws the client entities of the types that [`crate::EntityAssets`] holds with the entity
/// passes. The embedder inserts [`crate::EntityAssets`] when its pack and registries are ready,
/// and adds the terrain renderer, whose gbuffer and depth pyramid the passes use.
pub struct EntityRenderPlugin;

impl Plugin for EntityRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SlotArena>()
            .init_resource::<EntityClock>()
            .init_resource::<EntityVm>()
            .add_systems(
                FixedUpdate,
                advance_clock.before(ClientEntityTick).run_if(
                    |ticking: Option<Res<ServerTicking>>| {
                        ticking.is_none_or(|ticking| ticking.runs())
                    },
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    frame_clock,
                    assign_slots,
                    write_ticks,
                    write_layers,
                    write_inputs,
                )
                    .chain()
                    .in_set(EntitySlotSystems),
            )
            .add_observer(release_slot);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<EntityRenderState>()
            .add_systems(ExtractSchedule, extract_entities)
            .add_systems(Render, prepare_entities.in_set(RenderSystems::Prepare))
            .add_systems(Core3d, draw_entities.in_set(WorldPass::Entities));
    }
}
