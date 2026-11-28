use crate::batch_glint::{BatchGlint, batch_has_glint};
use crate::draw_gate::{gui_frame_changed, layer_stale};
use crate::glyph_cache::GlyphCache;
use crate::gui_batch::GuiBatch;
use crate::gui_batch_build::build_gui_batch;
use crate::gui_config::GuiConfig;
use crate::gui_fade::GuiFade;
use crate::gui_glint::{GuiGlint, write_gui_glint};
use crate::gui_images::GuiImages;
use crate::gui_layer::GuiLayer;
use crate::gui_layer::spawn_layer;
use crate::gui_phase::GuiPhase;
use crate::gui_pointer::GuiPointerPlugin;
use crate::item_icons::build_icon_meshes;
use crate::stitch_requests::StitchRequests;
use crate::tooltip::Tooltip;
use crate::tooltip_layer::{TooltipLayer, draw_tooltip};
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResourcePlugin;
use mcrs_minecraft_item_icon::IconMeshes;
use mcrs_minecraft_model::item_model_bake::ItemModels;

pub struct GuiPlugin {
    pub config: GuiConfig,
}

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config)
            .init_resource::<GuiBatch>()
            .init_resource::<BatchGlint>()
            .init_resource::<GuiGlint>()
            .init_resource::<GuiFade>()
            .init_resource::<GlyphCache>()
            .init_resource::<StitchRequests>()
            .add_plugins(GuiPointerPlugin)
            .add_plugins(ExtractResourcePlugin::<GuiImages>::default())
            .add_plugins(ExtractResourcePlugin::<GuiGlint>::default())
            .add_systems(Startup, spawn_layer::<TooltipLayer>)
            .add_systems(
                Update,
                build_icon_meshes.run_if(resource_exists_and_changed::<ItemModels>),
            )
            .configure_sets(PostUpdate, (GuiPhase::Draw, GuiPhase::Batch).chain())
            .add_systems(
                PostUpdate,
                (
                    draw_tooltip
                        .in_set(GuiPhase::Batch)
                        .before(build_gui_batch)
                        .run_if(
                            layer_stale::<TooltipLayer>()
                                .or_else(any_match_filter::<Changed<Tooltip>>)
                                .or_else(any_component_removed::<Tooltip>),
                        ),
                    build_gui_batch.in_set(GuiPhase::Batch).run_if(
                        any_match_filter::<Changed<GuiLayer>>
                            .or_else(any_component_removed::<GuiLayer>)
                            .or_else(resource_changed::<GuiFade>)
                            .or_else(resource_changed::<GuiConfig>)
                            .or_else(gui_frame_changed())
                            .or_else(resource_exists_and_changed::<ItemModels>)
                            .or_else(resource_exists_and_changed::<IconMeshes>),
                    ),
                    write_gui_glint
                        .in_set(GuiPhase::Batch)
                        .after(build_gui_batch)
                        .run_if(batch_has_glint),
                ),
            );
    }
}
