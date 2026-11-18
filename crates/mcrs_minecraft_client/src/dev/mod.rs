use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor, WindowPosition};
use mcrs_minecraft_render::DrawMask;
use mcrs_minecraft_render::sky::SkyDrawsOnly;

use crate::config::Knobs;
use crate::stream;
use crate::terrain_debug_entry;
use mcrs_minecraft_client_gui::debug_screen::{DebugScreenSet, collecting};
use mcrs_minecraft_client_gui::{debug_entry_fps, debug_entry_system_specs};
use mcrs_minecraft_client_world::columns::ClientTerrainSet;
use mcrs_minecraft_client_world::local_player::LocalPlayerTick;

pub mod census;
pub mod chunk_guard;
pub mod entry_frame;
pub mod flight;
#[cfg(target_os = "macos")]
pub mod gputrace;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
pub mod join_menu;
pub mod knobs;
pub mod light_guard;
pub mod play_world;
pub mod raster;
pub mod traces;
#[cfg(feature = "telemetry-tracy")]
pub mod tracy_preview;

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        let knobs = Knobs::of(app);
        app.add_plugins(raster::RasterPlugin).insert_resource(
            knobs
                .raster_fraction()
                .map(raster::Raster)
                .unwrap_or_default(),
        );
        if let Some(bits) = knobs.draw_mask() {
            app.insert_resource(DrawMask(bits));
        }
        if let Some(only) = knobs.sky_draws_only() {
            app.insert_resource(SkyDrawsOnly(only));
        }
        #[cfg(target_os = "macos")]
        app.insert_resource(gputrace::AutoTrace(knobs.gputrace_path()))
            .add_systems(Update, gputrace::gputrace);
        #[cfg(not(target_family = "wasm"))]
        {
            if let Some(level) = knobs.light_guard() {
                app.add_plugins(light_guard::LightGuardPlugin { level });
            }
            if let Some(level) = knobs.chunk_guard() {
                app.add_plugins(chunk_guard::ChunkGuardPlugin { level });
            }
            if knobs.hot_clocks() {
                std::thread::Builder::new()
                    .name("hot clocks".into())
                    .spawn(|| {
                        loop {
                            std::hint::spin_loop();
                        }
                    })
                    .expect("a thread");
            }
            if !knobs.monitor_primary() {
                app.add_systems(PostStartup, place_window);
            }
        }
        if let Some(seconds) = knobs.census_interval() {
            app.insert_resource(census::Census::every(seconds))
                .add_systems(Update, census::census);
        }
        #[cfg(feature = "telemetry-tracy")]
        if knobs.tracy_preview() {
            app.init_resource::<tracy_preview::Preview>()
                .add_systems(Update, tracy_preview::request);
        }
        app.add_systems(
            Update,
            traces::record_traces
                .in_set(ClientTerrainSet::Build)
                .after(stream::flush_streams)
                .run_if(stream::can_stream),
        )
        .insert_resource(entry_frame::UploadBudget(knobs.upload_budget()))
        .add_systems(
            Update,
            (
                entry_frame::display
                    .after(debug_entry_fps::display)
                    .before(debug_entry_fps::display_version),
                entry_frame::display_triangles
                    .after(debug_entry_system_specs::display)
                    .before(terrain_debug_entry::display)
                    .run_if(resource_exists::<stream::Loader>),
            )
                .in_set(DebugScreenSet::Collect)
                .run_if(collecting),
        );
    }
}

pub fn script_flight(app: &mut App) {
    let knobs = Knobs::of(app);
    let Some(speed) = knobs.scripted_flight() else {
        return;
    };
    app.insert_resource(flight::ScriptedFlight {
        flying_speed: speed,
        turn_at: knobs.turn_after(),
        turned: false,
    })
    .add_systems(
        FixedUpdate,
        flight::fly_scripted
            .after(crate::input::read_movement_keys)
            .before(LocalPlayerTick),
    );
}

pub fn place_window(
    monitors: Query<(&Monitor, Has<PrimaryMonitor>)>,
    window: Option<Single<&mut Window>>,
) {
    let Some(mut window) = window else {
        return;
    };
    let Some((monitor, _)) = monitors
        .iter()
        .max_by_key(|(monitor, primary)| (monitor.refresh_rate_millihertz, *primary))
    else {
        return;
    };
    let free = IVec2::new(
        monitor.physical_width as i32 - window.resolution.physical_width() as i32,
        monitor.physical_height as i32 - window.resolution.physical_height() as i32,
    );
    window.position = WindowPosition::At(monitor.physical_position + free.max(IVec2::ZERO) / 2);
    info!(
        name = monitor.name.as_deref().unwrap_or("?"),
        hz = monitor.refresh_rate_millihertz.map(|hz| hz as f32 / 1000.0),
        at = ?window.position,
        "window monitor"
    );
}
