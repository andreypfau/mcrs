use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::ecs::system::ScheduleSystem;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use mcrs_minecraft_core::resource_location::ResourceLocation;

use crate::gui::debug_screen_overlay;

pub mod displayer;
pub mod entry_day_count;
pub mod entry_fps;
pub mod entry_position;
pub mod entry_section_position;
pub mod entry_system_specs;
#[cfg(not(target_family = "wasm"))]
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

    pub fn is_enabled(&self, id: DebugEntryId) -> bool {
        match self.status(id) {
            DebugScreenEntryStatus::AlwaysOn => true,
            DebugScreenEntryStatus::InOverlay => self.overlay_visible,
            DebugScreenEntryStatus::Never => false,
        }
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
            entry
                .in_set(set)
                .in_set(DebugScreenSet::Collect)
                .run_if(move |list: Res<DebugScreenEntryList>| list.is_enabled(id)),
        )
    }
}

pub struct DebugScreenEntries;

impl DebugScreenEntries {
    pub const DAY_COUNT: DebugEntryId = ResourceLocation::new_static("minecraft:day_count");
    pub const FPS: DebugEntryId = ResourceLocation::new_static("minecraft:fps");
    pub const GAME_VERSION: DebugEntryId = ResourceLocation::new_static("minecraft:game_version");
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
            Self::GAME_VERSION,
            DebugScreenEntryStatus::InOverlay,
            entry_version::display,
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
        );

        #[cfg(not(target_family = "wasm"))]
        app.add_debug_screen_entry(
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
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.init_resource::<DebugScreenEntryList>()
            .init_resource::<DebugScreenDisplayer>()
            .configure_sets(
                Update,
                (
                    DebugScreenSet::Clear,
                    DebugScreenSet::Collect,
                    DebugScreenSet::Render,
                )
                    .chain(),
            )
            .add_systems(Startup, debug_screen_overlay::spawn)
            .add_systems(
                Update,
                (
                    debug_screen_overlay::toggle_overlay.before(DebugScreenSet::Clear),
                    clear_displayer.in_set(DebugScreenSet::Clear),
                    debug_screen_overlay::render.in_set(DebugScreenSet::Render),
                ),
            );
        DebugScreenEntries::register(app);
    }
}

fn clear_displayer(mut displayer: ResMut<DebugScreenDisplayer>) {
    displayer.clear();
}
