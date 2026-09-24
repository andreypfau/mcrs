use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::{ExtractSchedule, RenderApp};
use bevy::ui::UiSystems;
use bevy::ui_render::RenderUiSystems;

use crate::gui::chunk_map::ChunkMap;
use crate::gui::debug_screen_overlay;
use crate::gui::light_levels::LightLevels;

/// Whether any panel is up this frame and whether one was last frame. Bevy's UI systems run
/// every frame whatever is on screen; with nothing shown they are a fixed cost of the frame,
/// so they run only while this says so, and one frame longer so a hidden panel gets extracted
/// as hidden.
#[derive(Resource, Clone, Copy, Default, ExtractResource)]
pub struct UiNeeded {
    now: bool,
    before: bool,
}

impl UiNeeded {
    fn wanted(&self) -> bool {
        self.now || self.before
    }
}

fn ui_needed(needed: Option<Res<UiNeeded>>) -> bool {
    needed.is_some_and(|needed| needed.wanted())
}

fn update_ui_needed(
    overlay: Res<DebugOverlay>,
    map: Res<ChunkMap>,
    levels: Res<LightLevels>,
    mut needed: ResMut<UiNeeded>,
) {
    let now = **overlay || map.visible() || levels.visible();
    let next = UiNeeded {
        now,
        before: needed.now,
    };
    if (next.now, next.before) != (needed.now, needed.before) {
        *needed = next;
    }
}

pub mod displayer;
pub mod entry_fps;
pub mod entry_frame;
pub mod entry_network;
pub mod entry_position;
pub mod entry_system_specs;
pub mod entry_terrain;

pub use displayer::DebugScreenDisplayer;

/// Frames the entries actually run on: a shown overlay refreshes on a timer rather than every
/// frame, and a stats log collects on its own timer with the overlay staying hidden, so the
/// headline numbers never include the overlay's own cost.
#[derive(Resource)]
pub struct Refresh {
    active: bool,
    logging: bool,
    log_every: Option<f32>,
    next_refresh: f32,
    next_log: f32,
}

impl Refresh {
    const SHOWN_INTERVAL: f32 = 0.1;

    fn new(log_every: Option<f32>) -> Self {
        Self {
            active: false,
            logging: false,
            log_every,
            next_refresh: 0.0,
            next_log: 0.0,
        }
    }
}

/// Whether the F3 overlay is up.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct DebugOverlay(bool);

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DebugScreenSet {
    Clear,
    Collect,
    Render,
}

pub struct DebugScreenPlugin;

impl Plugin for DebugScreenPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::new(entry_fps::HISTORY));
        }
        app.init_resource::<DebugOverlay>()
            .init_resource::<mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink>()
            .init_resource::<DebugScreenDisplayer>()
            .init_resource::<UiNeeded>()
            .add_plugins(ExtractResourcePlugin::<UiNeeded>::default())
            .add_systems(Last, update_ui_needed)
            .configure_sets(PreUpdate, UiSystems::Focus.run_if(ui_needed))
            .configure_sets(
                PostUpdate,
                (
                    UiSystems::Prepare,
                    UiSystems::Propagate,
                    UiSystems::Content,
                    UiSystems::Layout,
                    UiSystems::PostLayout,
                    UiSystems::Stack,
                )
                    .run_if(ui_needed),
            )
            .insert_resource(Refresh::new(crate::config::stats_interval()))
            .configure_sets(
                Update,
                (
                    DebugScreenSet::Clear,
                    DebugScreenSet::Collect,
                    DebugScreenSet::Render,
                )
                    .chain()
                    .run_if(|refresh: Res<Refresh>| refresh.active),
            )
            .init_resource::<debug_screen_overlay::DebugModifier>()
            .add_systems(Startup, debug_screen_overlay::spawn)
            .add_systems(
                Update,
                (
                    (debug_screen_overlay::toggle_overlay, schedule_refresh)
                        .chain()
                        .before(DebugScreenSet::Clear),
                    clear_displayer.in_set(DebugScreenSet::Clear),
                    (
                        debug_screen_overlay::render,
                        log_debug_screen.run_if(|refresh: Res<Refresh>| refresh.logging),
                    )
                        .in_set(DebugScreenSet::Render),
                ),
            );
        // Vanilla runs its entries in identifier order, so the columns keep the same layout
        // frame to frame.
        app.add_systems(
            Update,
            (
                entry_fps::display,
                entry_frame::display,
                entry_fps::display_version,
                entry_network::display,
                entry_position::display,
                entry_system_specs::display,
                entry_terrain::display.run_if(resource_exists::<crate::stream::Loader>),
            )
                .chain()
                .in_set(DebugScreenSet::Collect)
                .run_if(|overlay: Res<DebugOverlay>, refresh: Res<Refresh>| {
                    **overlay || refresh.logging
                }),
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.configure_sets(
                ExtractSchedule,
                (
                    RenderUiSystems::ExtractCameraViews,
                    RenderUiSystems::ExtractBoxShadows,
                    RenderUiSystems::ExtractBackgrounds,
                    RenderUiSystems::ExtractImages,
                    RenderUiSystems::ExtractTextureSlice,
                    RenderUiSystems::ExtractBorders,
                    RenderUiSystems::ExtractViewportNodes,
                    RenderUiSystems::ExtractTextBackgrounds,
                    RenderUiSystems::ExtractTextShadows,
                    RenderUiSystems::ExtractText,
                    RenderUiSystems::ExtractCursor,
                    RenderUiSystems::ExtractDebug,
                    RenderUiSystems::ExtractGradient,
                )
                    .run_if(ui_needed),
            );
        }
    }
}

fn schedule_refresh(mut refresh: ResMut<Refresh>, overlay: Res<DebugOverlay>, time: Res<Time>) {
    let now = time.elapsed_secs();
    let refresh = refresh.bypass_change_detection();
    let shown = **overlay && now >= refresh.next_refresh;
    if shown {
        refresh.next_refresh = now + Refresh::SHOWN_INTERVAL;
    }
    refresh.logging = refresh.log_every.is_some_and(|every| {
        let due = now >= refresh.next_log;
        if due {
            refresh.next_log = now + every;
        }
        due
    });
    refresh.active = shown || refresh.logging || overlay.is_changed();
}

fn log_debug_screen(displayer: Res<DebugScreenDisplayer>) {
    let (left, right) = displayer.columns();
    let line = left
        .into_iter()
        .chain(right)
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    info!(target: "mcrs_client::stats", "{line}");
}

fn clear_displayer(mut displayer: ResMut<DebugScreenDisplayer>) {
    displayer.clear();
}
