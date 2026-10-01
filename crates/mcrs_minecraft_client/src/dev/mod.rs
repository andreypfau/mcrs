use std::num::NonZeroU32;

use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor, WindowPosition};
use mcrs_minecraft_render::DrawMask;
use mcrs_minecraft_render::sky::SkyDrawsOnly;
use mcrs_minecraft_world::entity::player::FlyingSpeed;

use crate::columns::ClientTerrainSet;
use crate::gui::debug::{DebugScreenSet, collecting, entry_fps, entry_system_specs, entry_terrain};
use crate::stream;

pub mod census;
pub mod chunk_guard;
pub mod entry_frame;
pub mod flight;
#[cfg(target_os = "macos")]
pub mod gputrace;
pub mod knobs;
pub mod light_guard;
pub mod raster;
pub mod traces;
#[cfg(feature = "telemetry-tracy")]
pub mod tracy_preview;

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(raster::RasterPlugin).insert_resource(
            knobs::raster_fraction()
                .map(raster::Raster)
                .unwrap_or_default(),
        );
        if let Some(bits) = knobs::draw_mask() {
            app.insert_resource(DrawMask(bits));
        }
        if let Some(only) = knobs::sky_draws_only() {
            app.insert_resource(SkyDrawsOnly(only));
        }
        #[cfg(target_os = "macos")]
        app.insert_resource(gputrace::AutoTrace(knobs::gputrace_path()))
            .add_systems(Update, gputrace::gputrace);
        #[cfg(not(target_family = "wasm"))]
        {
            if let Some(level) = knobs::light_guard() {
                app.add_plugins(light_guard::LightGuardPlugin { level });
            }
            if let Some(level) = knobs::chunk_guard() {
                app.add_plugins(chunk_guard::ChunkGuardPlugin { level });
            }
            if knobs::hot_clocks() {
                std::thread::Builder::new()
                    .name("hot clocks".into())
                    .spawn(|| {
                        loop {
                            std::hint::spin_loop();
                        }
                    })
                    .expect("a thread");
            }
            if !knobs::monitor_primary() {
                app.add_systems(PostStartup, place_window);
            }
        }
        if let Some(seconds) = knobs::census_interval() {
            app.insert_resource(census::Census::every(seconds))
                .add_systems(Update, census::census);
        }
        #[cfg(feature = "telemetry-tracy")]
        if knobs::tracy_preview() {
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
        .insert_resource(entry_frame::UploadBudget(crate::config::upload_budget()))
        .add_systems(
            Update,
            (
                entry_frame::display
                    .after(entry_fps::display)
                    .before(entry_fps::display_version),
                entry_frame::display_triangles
                    .after(entry_system_specs::display)
                    .before(entry_terrain::display)
                    .run_if(resource_exists::<stream::Loader>),
            )
                .in_set(DebugScreenSet::Collect)
                .run_if(collecting),
        );
    }
}

pub fn frame_latency() -> Option<NonZeroU32> {
    knobs::frame_latency()
}

pub fn script_flight(app: &mut App, player: Entity) {
    let Some(speed) = knobs::scripted_flight() else {
        return;
    };
    app.insert_resource(flight::ScriptedFlight {
        turn_at: knobs::turn_after(),
        turned: false,
    });
    app.world_mut()
        .entity_mut(player)
        .insert(FlyingSpeed(speed));
}

/// Opens the window on the fastest display rather than on the system's primary
/// one.
///
/// Whenever the engine is quicker than the display, the frame rate a run reads
/// is the display's: a 60 Hz external panel pins a whole chunk load to sixty
/// frames a second and everything paced per frame along with it, while the
/// laptop's own panel does a hundred and twenty. `MCRS_MONITOR=primary` puts it
/// back. The placement is absolute rather than a `MonitorSelection`, which macOS
/// ignores once the window exists — which is also why `FULLSCREEN=1` still takes
/// over the primary display and not this one.
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
