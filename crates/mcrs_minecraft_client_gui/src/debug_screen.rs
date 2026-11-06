use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::{ExtractSchedule, RenderApp};
use bevy::ui::UiSystems;
use bevy::ui_render::RenderUiSystems;

use crate::chunk_map::ChunkMap;
use crate::debug_screen_overlay;
use crate::light_levels::LightLevels;

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
    map: Option<Res<ChunkMap>>,
    levels: Option<Res<LightLevels>>,
    mut needed: ResMut<UiNeeded>,
) {
    let now = **overlay
        || map.is_some_and(|map| map.visible())
        || levels.is_some_and(|levels| levels.visible());
    let next = UiNeeded {
        now,
        before: needed.now,
    };
    if (next.now, next.before) != (needed.now, needed.before) {
        *needed = next;
    }
}

use crate::debug_displayer::DebugScreenDisplayer;

/// Frames the entries actually run on: a shown overlay refreshes on a timer rather than every
/// frame, and a stats log collects on its own timer with the overlay staying hidden, so the
/// headline numbers never include the overlay's own cost.
#[derive(Resource)]
pub struct Refresh {
    active: bool,
    logging: bool,
    log_every: Option<f32>,
    always_timed: bool,
    next_refresh: f32,
    next_log: f32,
}

impl Refresh {
    const SHOWN_INTERVAL: f32 = 0.1;

    fn new(log_every: Option<f32>, always_timed: bool) -> Self {
        Self {
            active: false,
            logging: false,
            log_every,
            always_timed,
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

pub struct DebugScreenPlugin {
    /// Seconds between debug-screen lines written to the log, for a run whose window cannot be
    /// read.
    pub stats_interval: Option<f32>,
    pub always_time_passes: bool,
}

impl Plugin for DebugScreenPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::new(
                crate::debug_entry_fps::HISTORY,
            ));
        }
        app.init_resource::<DebugOverlay>()
            .init_resource::<mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink>(
            )
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
            .insert_resource(Refresh::new(self.stats_interval, self.always_time_passes))
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
                crate::debug_entry_fps::display,
                crate::debug_entry_fps::display_version,
                crate::debug_entry_network::display,
                crate::debug_entry_position::display,
                crate::debug_entry_system_specs::display,
            )
                .chain()
                .in_set(DebugScreenSet::Collect)
                .run_if(collecting),
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

/// Whether debug entries run this frame. False without [`DebugScreenPlugin`], so systems gated on
/// it can be added by apps that render without the debug screen.
pub fn collecting(overlay: Option<Res<DebugOverlay>>, refresh: Option<Res<Refresh>>) -> bool {
    overlay.is_some_and(|overlay| **overlay) || refresh.is_some_and(|refresh| refresh.logging)
}

fn schedule_refresh(
    mut refresh: ResMut<Refresh>,
    overlay: Res<DebugOverlay>,
    time: Res<Time>,
    gpu: Res<mcrs_minecraft_render_probe::probe::GpuTimings>,
) {
    gpu.want(refresh.always_timed || **overlay || refresh.log_every.is_some());
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

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    #[test]
    fn nothing_collects_without_the_debug_screen() {
        let mut world = World::new();
        assert!(!world.run_system_once(collecting).unwrap());
    }

    #[test]
    fn the_overlay_alone_needs_the_ui() {
        let mut world = World::new();
        world.insert_resource(DebugOverlay(true));
        world.init_resource::<UiNeeded>();
        world.run_system_once(update_ui_needed).unwrap();
        assert!(world.resource::<UiNeeded>().wanted());
    }
}
