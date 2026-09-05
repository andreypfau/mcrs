use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::ecs::system::ScheduleSystem;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::{ExtractSchedule, RenderApp};
use bevy::ui::UiSystems;
use bevy::ui_render::RenderUiSystems;
use mcrs_minecraft_core::resource_location::ResourceLocation;

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
    list: Res<DebugScreenEntryList>,
    map: Res<ChunkMap>,
    levels: Res<LightLevels>,
    mut needed: ResMut<UiNeeded>,
) {
    let now = list.overlay_visible() || map.visible() || levels.visible();
    let next = UiNeeded {
        now,
        before: needed.now,
    };
    if (next.now, next.before) != (needed.now, needed.before) {
        *needed = next;
    }
}

pub mod displayer;
pub mod entry_day_count;
pub mod entry_fps;
pub mod entry_frame;
pub mod entry_network;
pub mod entry_position;
pub mod entry_section_position;
pub mod entry_system_specs;
pub mod entry_terrain;
pub mod entry_version;

pub use displayer::DebugScreenDisplayer;

pub type DebugEntryId = ResourceLocation<&'static str>;
pub type DebugEntryGroup = ResourceLocation<&'static str>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DebugScreenEntryStatus {
    #[allow(
        dead_code,
        reason = "only a debug-options screen puts an entry into this state"
    )]
    AlwaysOn,
    InOverlay,
    Never,
}

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

/// Which debug entries the player has turned on, and whether the F3 overlay is
/// up. The set that actually runs this frame is read off it, never stored.
#[derive(Resource, Default)]
pub struct DebugScreenEntryList {
    statuses: HashMap<DebugEntryId, DebugScreenEntryStatus>,
    order: Vec<DebugEntryId>,
    overlay_visible: bool,
}

impl DebugScreenEntryList {
    pub fn status(&self, id: DebugEntryId) -> DebugScreenEntryStatus {
        self.statuses
            .get(&id)
            .copied()
            .unwrap_or(DebugScreenEntryStatus::Never)
    }

    pub fn set_status(&mut self, id: DebugEntryId, status: DebugScreenEntryStatus) {
        self.statuses.insert(id, status);
    }

    pub fn is_enabled(&self, id: DebugEntryId, refresh: &Refresh) -> bool {
        match self.status(id) {
            DebugScreenEntryStatus::AlwaysOn => true,
            DebugScreenEntryStatus::InOverlay => self.overlay_visible || refresh.logging,
            DebugScreenEntryStatus::Never => false,
        }
    }

    pub fn overlay_visible(&self) -> bool {
        self.overlay_visible
    }

    pub fn toggle_overlay(&mut self) {
        self.overlay_visible = !self.overlay_visible;
    }
}

/// Vanilla runs its entries in identifier order, so the columns keep the same
/// layout frame to frame; registration order stands in for that here.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
struct DebugEntrySet(DebugEntryId);

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DebugScreenSet {
    Clear,
    Collect,
    Render,
}

pub trait AddDebugScreenEntry {
    fn add_debug_screen_entry<M>(
        &mut self,
        id: DebugEntryId,
        status: DebugScreenEntryStatus,
        entry: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;
}

impl AddDebugScreenEntry for App {
    fn add_debug_screen_entry<M>(
        &mut self,
        id: DebugEntryId,
        status: DebugScreenEntryStatus,
        entry: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        let mut list = self.world_mut().resource_mut::<DebugScreenEntryList>();
        let previous = list.order.last().copied();
        list.set_status(id, status);
        list.order.push(id);

        let set = DebugEntrySet(id);
        if let Some(previous) = previous {
            self.configure_sets(Update, set.clone().after(DebugEntrySet(previous)));
        }
        self.add_systems(
            Update,
            entry.in_set(set).in_set(DebugScreenSet::Collect).run_if(
                move |list: Res<DebugScreenEntryList>, refresh: Res<Refresh>| {
                    list.is_enabled(id, &refresh)
                },
            ),
        )
    }
}

pub struct DebugScreenEntries;

impl DebugScreenEntries {
    pub const DAY_COUNT: DebugEntryId = ResourceLocation::new_static("minecraft:day_count");
    pub const FPS: DebugEntryId = ResourceLocation::new_static("minecraft:fps");
    pub const FRAME: DebugEntryId = ResourceLocation::new_static("mcrs:frame");
    pub const GAME_VERSION: DebugEntryId = ResourceLocation::new_static("minecraft:game_version");
    pub const NETWORK: DebugEntryId = ResourceLocation::new_static("mcrs:network");
    pub const PLAYER_POSITION: DebugEntryId =
        ResourceLocation::new_static("minecraft:player_position");
    pub const PLAYER_SECTION_POSITION: DebugEntryId =
        ResourceLocation::new_static("minecraft:player_section_position");
    pub const SYSTEM_SPECS: DebugEntryId = ResourceLocation::new_static("minecraft:system_specs");
    pub const TERRAIN: DebugEntryId = ResourceLocation::new_static("minecraft:terrain");

    /// Registration order is identifier order, and the status each entry starts
    /// on is the one the `default` profile gives it.
    fn register(app: &mut App) {
        app.add_debug_screen_entry(
            Self::DAY_COUNT,
            DebugScreenEntryStatus::Never,
            entry_day_count::display,
        )
        .add_debug_screen_entry(
            Self::FPS,
            DebugScreenEntryStatus::InOverlay,
            entry_fps::display,
        )
        .add_debug_screen_entry(
            Self::FRAME,
            DebugScreenEntryStatus::InOverlay,
            entry_frame::display,
        )
        .add_debug_screen_entry(
            Self::GAME_VERSION,
            DebugScreenEntryStatus::InOverlay,
            entry_version::display,
        )
        .add_debug_screen_entry(
            Self::NETWORK,
            DebugScreenEntryStatus::InOverlay,
            entry_network::display,
        )
        .add_debug_screen_entry(
            Self::PLAYER_POSITION,
            DebugScreenEntryStatus::InOverlay,
            entry_position::display,
        )
        .add_debug_screen_entry(
            Self::PLAYER_SECTION_POSITION,
            DebugScreenEntryStatus::InOverlay,
            entry_section_position::display,
        )
        .add_debug_screen_entry(
            Self::SYSTEM_SPECS,
            DebugScreenEntryStatus::InOverlay,
            entry_system_specs::display,
        )
        .add_debug_screen_entry(
            Self::TERRAIN,
            DebugScreenEntryStatus::InOverlay,
            entry_terrain::display.run_if(resource_exists::<crate::stream::Loader>),
        );
    }
}

pub struct DebugScreenPlugin;

impl Plugin for DebugScreenPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::new(entry_fps::HISTORY));
        }
        app.init_resource::<DebugScreenEntryList>()
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
        DebugScreenEntries::register(app);

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

fn schedule_refresh(
    mut refresh: ResMut<Refresh>,
    list: Res<DebugScreenEntryList>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let refresh = refresh.bypass_change_detection();
    let shown = list.overlay_visible() && now >= refresh.next_refresh;
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
    refresh.active = shown || refresh.logging || list.is_changed();
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
