use bevy::core_pipeline::schedule::Core3d;
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResourcePlugin;
use bevy::render::render_resource::BindGroup;
use bevy::render::renderer::{RenderAdapter, RenderDevice};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use mcrs_minecraft_render::sky::ExtractedSky;
use mcrs_minecraft_render::{LightTint, RenderPath, WorldPass};

use crate::colour;
use crate::volume::{self, VolumeQueue, VolumeSettings};

pub struct LightVolumeRenderPlugin {
    pub settings: VolumeSettings,
}

impl Plugin for LightVolumeRenderPlugin {
    fn build(&self, app: &mut App) {
        bevy::shader::load_shader_library!(app, "shaders/volume.wesl");
        bevy::asset::embedded_asset!(app, "shaders/colour.wgsl");
        let queue = VolumeQueue::default();
        app.insert_resource(self.settings)
            .insert_resource(queue)
            .add_plugins(ExtractResourcePlugin::<VolumeSettings>::default());
    }

    fn finish(&self, app: &mut App) {
        let world = app.world();
        match (
            world.get_resource::<RenderDevice>(),
            world.get_resource::<RenderAdapter>(),
        ) {
            (Some(device), Some(adapter)) if RenderPath::of(device, adapter).is_full() => {}
            (Some(_), Some(_)) => {
                info!("no compute passes: block light is drawn without a tint");
                return;
            }
            _ => {
                warn!("the light volume plugin finished before the renderer: no light tint");
                return;
            }
        }
        let queue = world.resource::<VolumeQueue>().clone();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(queue)
            .insert_resource(LightTint {
                layout: volume::volume_layout(),
                bind_group: None,
            })
            .add_systems(RenderStartup, colour::init_colour_pipelines)
            .add_systems(
                Render,
                (
                    colour::prepare_colour_pipelines
                        .run_if(colour::colour_pipelines_unsettled)
                        .in_set(RenderSystems::Prepare),
                    (
                        volume::apply_volume_commands.run_if(resource_exists::<volume::Volume>),
                        volume::write_volume_terms
                            .run_if(resource_exists::<volume::Volume>)
                            .run_if(resource_exists::<ExtractedSky>),
                    )
                        .chain()
                        .in_set(RenderSystems::Prepare),
                    (
                        volume::fit_volume.run_if(resource_exists_and_changed::<VolumeSettings>),
                        publish_tint.run_if(resource_exists_and_changed::<volume::Volume>),
                    )
                        .chain()
                        .in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(Core3d, colour::propagate_colour.in_set(WorldPass::Upload));
    }
}

fn publish_tint(volume: Option<Res<volume::Volume>>, mut tint: ResMut<LightTint>) {
    let bind_group = volume.as_deref().map(|volume| &volume.bind_group);
    if bind_group.map(BindGroup::id) != tint.bind_group.as_ref().map(BindGroup::id) {
        tint.bind_group = bind_group.cloned();
    }
}
