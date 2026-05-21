use bevy_app::{App, Plugin};

#[cfg(feature = "telemetry-diagnostics")]
use bevy_diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};

/// Tracy and diagnostics substrate for mcrs.
///
/// When the `telemetry-tracy` cargo feature is on, the Tracy subscriber is installed
/// by `bevy_log::LogPlugin` (which pulls `tracing-tracy` via its own feature gate).
/// Per-system tracing spans are emitted automatically by Bevy's `bevy_ecs/trace` cargo
/// feature, enabled workspace-wide. This plugin's `build()` body is therefore
/// intentionally empty under `telemetry-tracy`: every system invocation already
/// produces a span, and the project-supplied `#[instrument]` attributes at hot system
/// bodies (using a `module::function` naming convention — `lighting::propagate_decrease`,
/// `world::column_gen`, `network::process_received_packet`, etc.) layer onto that
/// substrate.
///
/// A wrapper helper that opens a Tracy zone on `SystemSet` entry and closes it on exit
/// was considered but is not reachable on Bevy 0.18: the public
/// `add_systems(..., wrapper.in_set(X))` form adds a sibling system inside the set, not
/// a parent that nests it. See `README.md` for the full engineering rationale.
///
/// Under `telemetry-diagnostics`, this plugin adds `FrameTimeDiagnosticsPlugin` and
/// `EntityCountDiagnosticsPlugin` so frame-time and entity-count data surface alongside
/// Tracy zones in the same capture.
pub struct TelemetryPlugin;

impl Plugin for TelemetryPlugin {
    #[allow(unused_variables)]
    fn build(&self, app: &mut App) {
        #[cfg(feature = "telemetry-diagnostics")]
        {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
            app.add_plugins(EntityCountDiagnosticsPlugin::default());
        }
    }
}
