use bevy::core_pipeline::schedule::Core3d;
use bevy::prelude::*;
use bevy::render::renderer::{RenderAdapter, RenderDevice};
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSystems};
use mcrs_minecraft_entity_visual::{ClientEntityTick, EntityLightSystems};
use mcrs_minecraft_render::{RenderPath, WorldPass};

use crate::gpu_layouts::passes_fit;
use crate::main_world::{
    EntityClock, EntityVm, advance_clock, assign_slots, frame_clock, release_slot, server_ticks,
    write_inputs, write_layers, write_ticks,
};
use crate::player_skin::{SkinSlots, assign_skins, release_skin};
use crate::render_world::{EntityRenderState, draw_entities, extract_entities, prepare_entities};
use crate::slot_upload::SlotArena;

/// The systems that keep the slot of each client entity, in `PostUpdate`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntitySlotSystems;

/// Whether this device runs the entity passes, in the main world and the render world alike.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntityPasses(bool);

#[cfg(test)]
impl EntityPasses {
    pub(crate) const ON: Self = Self(true);
}

/// A run condition: the device runs the entity passes.
pub fn entity_passes(passes: Option<Res<EntityPasses>>) -> bool {
    passes.is_some_and(|passes| passes.0)
}

/// Draws the client entities of the types that [`crate::EntityAssets`] holds with the entity
/// passes. The embedder inserts [`crate::EntityAssets`] when its pack and registries are ready,
/// and adds the terrain renderer, whose gbuffer and depth pyramid the passes use.
// chisle: the passes need compute and storage buffers in the vertex stage, so on WebGL2 and in
// WebGPU compatibility mode the plugin draws nothing; `docs/entity-render.md`, section 6, lifts it.
pub struct EntityRenderPlugin;

impl Plugin for EntityRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SlotArena>()
            .init_resource::<EntityClock>()
            .init_resource::<EntityVm>()
            .init_resource::<SkinSlots>()
            .add_systems(
                FixedUpdate,
                advance_clock.before(ClientEntityTick).run_if(server_ticks),
            )
            .add_systems(
                PostUpdate,
                (
                    frame_clock,
                    assign_slots,
                    assign_skins,
                    write_ticks,
                    write_layers,
                    write_inputs,
                )
                    .chain()
                    .in_set(EntitySlotSystems),
            )
            .configure_sets(
                PostUpdate,
                EntitySlotSystems
                    .after(EntityLightSystems)
                    .run_if(entity_passes),
            )
            .add_observer(release_slot)
            .add_observer(release_skin);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<EntityRenderState>()
            .add_systems(ExtractSchedule, extract_entities.run_if(entity_passes))
            .add_systems(
                Render,
                prepare_entities
                    .in_set(RenderSystems::Prepare)
                    .run_if(entity_passes),
            )
            .add_systems(
                Core3d,
                draw_entities
                    .in_set(WorldPass::Entities)
                    .run_if(entity_passes),
            );
    }

    fn finish(&self, app: &mut App) {
        let world = app.world();
        let passes = match (
            world.get_resource::<RenderDevice>(),
            world.get_resource::<RenderAdapter>(),
        ) {
            (Some(device), Some(adapter)) => {
                RenderPath::of(device, adapter).is_full() && passes_fit(&device.limits())
            }
            _ => {
                warn!("the entity plugin finished before the renderer: entities are not drawn");
                false
            }
        };
        if !passes {
            info!("this device draws no entity: it cannot bind the entity passes");
        }
        app.insert_resource(EntityPasses(passes));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(EntityPasses(passes));
        }
    }
}
