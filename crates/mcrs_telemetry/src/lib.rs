use bevy_app::{App, Plugin};

#[cfg(feature = "telemetry-diagnostics")]
use bevy_diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};

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
